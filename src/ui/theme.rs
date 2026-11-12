//! Colour themes and what the terminal can show.
//!
//! Masks speak in the dark theme's colours (VS Code Dark+). A theme maps each
//! of those to its own colour when drawing, and the result is brought down to
//! the terminal's depth: 24-bit where it is supported, else the nearest of the
//! 256 xterm colours (Apple's Terminal before macOS 26) or of the 16 ANSI ones.

use std::sync::RwLock;

use ratatui::style::Color;

pub use crate::state::THEMES;

/// How many colours the terminal shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    TrueColor,
    Ansi256,
    Ansi16,
}

/// The depth the environment advertises (`COLORTERM`, `TERM`).
pub fn detect_depth(colorterm: Option<&str>, term: Option<&str>) -> Depth {
    let colorterm = colorterm.unwrap_or("").to_lowercase();
    let term = term.unwrap_or("").to_lowercase();
    if colorterm == "truecolor"
        || colorterm == "24bit"
        || term.contains("truecolor")
        || term.contains("direct")
    {
        Depth::TrueColor
    } else if term.contains("256") || cfg!(windows) {
        // Windows Terminal and conhost both take at least 256 colours
        if cfg!(windows) {
            Depth::TrueColor
        } else {
            Depth::Ansi256
        }
    } else {
        Depth::Ansi16
    }
}

pub fn detect_depth_from_env() -> Depth {
    detect_depth(
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TERM").ok().as_deref(),
    )
}

/// The dark theme's colours, as the masks write them, and what each means.
const DARK: &[(&str, &str)] = &[
    ("#1e1e1e", "background"),
    ("#d4d4d4", "text"),
    ("#858585", "gutter"),
    ("#6a9955", "comment"),
    ("#569cd6", "keyword"),
    ("#c586c0", "control"),
    ("#dcdcaa", "function"),
    ("#4ec9b0", "type"),
    ("#ce9178", "string"),
    ("#b5cea8", "number"),
    ("#9cdcfe", "variable"),
    ("#4fc1ff", "constant"),
    ("#3fb950", "green"),
    ("#d29922", "yellow"),
    ("#f85149", "red"),
    ("#58a6ff", "blue"),
    ("#39c5cf", "cyan"),
    ("#6e7681", "dim"),
    ("#1f3d27", "added background"),
    ("#4b1f22", "removed background"),
    ("#252526", "panel background"),
    ("#cccccc", "panel text"),
    ("#3c3c3c", "panel border"),
];

/// A colour in a theme: a 24-bit value, a terminal palette colour, or the terminal's own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paint {
    Hex(u32),
    Named(Color),
    Default,
}

use Paint::{Default as Term, Hex, Named};

/// Light: VS Code Light+ code colours with GitHub's terminal and diff colours.
fn light(role: &str) -> Paint {
    Hex(match role {
        "background" => 0xffffff,
        "text" | "panel text" => 0x1f2328,
        "gutter" | "dim" => 0x6e7781,
        "comment" => 0x008000,
        "keyword" => 0x0000ff,
        "control" => 0xaf00db,
        "function" => 0x795e26,
        "type" => 0x267f99,
        "string" => 0xa31515,
        "number" => 0x098658,
        "variable" => 0x001080,
        "constant" => 0x0070c1,
        "green" => 0x1a7f37,
        "yellow" => 0x9a6700,
        "red" => 0xcf222e,
        "blue" => 0x0969da,
        "cyan" => 0x1b7c83,
        "added background" => 0xdafbe1,
        "removed background" => 0xffebe9,
        "panel background" => 0xf6f8fa,
        "panel border" => 0xd0d7de,
        _ => return Term,
    })
}

/// Colour-blind friendly: blue for added / passing, orange for removed / failing.
fn daltonized(role: &str, dark: bool) -> Option<Paint> {
    Some(Hex(match (role, dark) {
        ("green", true) => 0x58a6ff,
        ("red", true) => 0xf0883e,
        ("added background", true) => 0x12304a,
        ("removed background", true) => 0x4a2a14,
        ("comment", true) => 0x8b949e,
        ("green", false) => 0x0969da,
        ("red", false) => 0xbc4c00,
        ("added background", false) => 0xddf4ff,
        ("removed background", false) => 0xfff1e5,
        ("comment", false) => 0x57606a,
        _ => return None,
    }))
}

/// Only the terminal's 16 colours, so its own palette decides the shades.
fn ansi(role: &str, dark: bool) -> Paint {
    let bright = |c: Color, dim: Color| Named(if dark { c } else { dim });
    match role {
        "background" | "text" | "panel text" | "panel background" => Term,
        "gutter" | "dim" | "panel border" => Named(Color::DarkGray),
        "comment" | "green" => Named(Color::Green),
        "keyword" | "blue" => Named(Color::Blue),
        "control" => Named(Color::Magenta),
        "function" | "yellow" => Named(Color::Yellow),
        "type" | "cyan" => Named(Color::Cyan),
        "string" => bright(Color::LightRed, Color::Red),
        "number" => bright(Color::LightGreen, Color::Green),
        "variable" => bright(Color::LightBlue, Color::Blue),
        "constant" => bright(Color::LightCyan, Color::Cyan),
        "red" => Named(Color::Red),
        "added background" => Named(Color::Green),
        "removed background" => Named(Color::Red),
        _ => Term,
    }
}

/// The colour `hex` (a dark-theme colour) has in `theme`; other hex values pass through.
fn paint(theme: &str, hex: u32) -> Paint {
    let key = format!("#{hex:06x}");
    let Some((_, role)) = DARK.iter().find(|(h, _)| *h == key) else {
        return Hex(hex);
    };
    match theme {
        "light" => light(role),
        "dark-daltonized" => daltonized(role, true).unwrap_or(Hex(hex)),
        "light-daltonized" => daltonized(role, false).unwrap_or_else(|| light(role)),
        "dark-ansi" => ansi(role, true),
        "light-ansi" => ansi(role, false),
        _ => Hex(hex),
    }
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

fn rgb(hex: u32) -> (i32, i32, i32) {
    (
        (hex >> 16 & 0xff) as i32,
        (hex >> 8 & 0xff) as i32,
        (hex & 0xff) as i32,
    )
}

fn distance(a: (i32, i32, i32), b: (i32, i32, i32)) -> i32 {
    // weighted towards green, where the eye is most sensitive
    2 * (a.0 - b.0).pow(2) + 4 * (a.1 - b.1).pow(2) + 3 * (a.2 - b.2).pow(2)
}

/// The nearest of the 240 xterm cube and grey colours (16-255).
pub fn nearest_256(hex: u32) -> u8 {
    let c = rgb(hex);
    let level = |v: i32| {
        (0..6)
            .min_by_key(|&i| (CUBE[i] as i32 - v).abs())
            .expect("six levels")
    };
    let (r, g, b) = (level(c.0), level(c.1), level(c.2));
    let cube = 16 + 36 * r + 6 * g + b;
    let cube_rgb = (CUBE[r] as i32, CUBE[g] as i32, CUBE[b] as i32);
    let grey_i = ((c.0 + c.1 + c.2) / 3 - 8).clamp(0, 230) / 10;
    let grey_v = 8 + 10 * grey_i;
    let grey = 232 + grey_i as usize;
    if distance(c, (grey_v, grey_v, grey_v)) < distance(c, cube_rgb) {
        grey as u8
    } else {
        cube as u8
    }
}

/// The ANSI colour a `#rrggbb` reads as: by hue when it has colour, by lightness when grey.
///
/// Plain RGB distance sends muted editor colours (like `#3fb950`) to grey, so
/// a coloured value is placed by its hue and made bright when it is light.
pub fn nearest_16(hex: u32) -> Color {
    let (r, g, b) = rgb(hex);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = max - min;
    if chroma < 40 || max == 0 {
        return match max {
            0..=50 => Color::Black,
            51..=140 => Color::DarkGray,
            141..=220 => Color::Gray,
            _ => Color::White,
        };
    }
    let (rf, gf, bf, c) = (r as f32, g as f32, b as f32, chroma as f32);
    let hue = if max == r {
        60.0 * ((gf - bf) / c).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((bf - rf) / c + 2.0)
    } else {
        60.0 * ((rf - gf) / c + 4.0)
    };
    let bright = max > 200;
    let pick = |dim: Color, light: Color| if bright { light } else { dim };
    match hue as u32 {
        0..=20 | 340..=360 => pick(Color::Red, Color::LightRed),
        21..=70 => pick(Color::Yellow, Color::LightYellow),
        71..=160 => pick(Color::Green, Color::LightGreen),
        161..=200 => pick(Color::Cyan, Color::LightCyan),
        201..=260 => pick(Color::Blue, Color::LightBlue),
        _ => pick(Color::Magenta, Color::LightMagenta),
    }
}

/// A mask colour as the terminal should get it, in `theme` at `depth`.
pub fn color(theme: &str, depth: Depth, hex: u32) -> Option<Color> {
    match paint(theme, hex) {
        Term => None,
        Named(c) => Some(c),
        Hex(h) => Some(match depth {
            Depth::TrueColor => {
                let (r, g, b) = rgb(h);
                Color::Rgb(r as u8, g as u8, b as u8)
            }
            Depth::Ansi256 => Color::Indexed(nearest_256(h)),
            Depth::Ansi16 => nearest_16(h),
        }),
    }
}

static CURRENT: RwLock<Option<(String, Depth)>> = RwLock::new(None);

/// Use `theme` (and `depth`) for everything drawn from now on.
pub fn set(theme: &str, depth: Depth) {
    let name = if THEMES.contains(&theme) {
        theme
    } else {
        "dark"
    };
    if let Ok(mut c) = CURRENT.write() {
        *c = Some((name.to_string(), depth));
    }
}

/// The theme and depth in use (dark, 24-bit until `set` is called).
pub fn current() -> (String, Depth) {
    CURRENT
        .read()
        .ok()
        .and_then(|c| c.clone())
        .unwrap_or_else(|| ("dark".into(), Depth::TrueColor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_comes_from_colorterm_and_term() {
        assert_eq!(
            detect_depth(Some("truecolor"), Some("xterm-256color")),
            Depth::TrueColor
        );
        assert_eq!(detect_depth(Some("24bit"), None), Depth::TrueColor);
        if !cfg!(windows) {
            // Apple's Terminal before macOS 26: no COLORTERM, xterm-256color
            assert_eq!(detect_depth(None, Some("xterm-256color")), Depth::Ansi256);
            assert_eq!(detect_depth(None, Some("xterm")), Depth::Ansi16);
        }
    }

    #[test]
    fn dark_keeps_its_colours_and_light_swaps_them() {
        assert_eq!(
            color("dark", Depth::TrueColor, 0x6a9955),
            Some(Color::Rgb(0x6a, 0x99, 0x55))
        );
        assert_eq!(
            color("light", Depth::TrueColor, 0x6a9955),
            Some(Color::Rgb(0x00, 0x80, 0x00))
        );
        assert_eq!(
            color("light", Depth::TrueColor, 0x123456),
            Some(Color::Rgb(0x12, 0x34, 0x56))
        ); // not a theme colour
    }

    #[test]
    fn daltonized_themes_avoid_red_against_green() {
        for theme in ["dark-daltonized", "light-daltonized"] {
            let (added, removed) = (
                color(theme, Depth::TrueColor, 0x3fb950),
                color(theme, Depth::TrueColor, 0xf85149),
            );
            let Some(Color::Rgb(r, g, b)) = added else {
                panic!()
            };
            assert!(b > r && b > g, "{theme}: added is blue, {added:?}");
            let Some(Color::Rgb(r, _, b)) = removed else {
                panic!()
            };
            assert!(r > b, "{theme}: removed is orange, {removed:?}");
        }
    }

    #[test]
    fn ansi_themes_use_the_terminal_palette() {
        assert_eq!(
            color("dark-ansi", Depth::TrueColor, 0x3fb950),
            Some(Color::Green)
        );
        assert_eq!(
            color("light-ansi", Depth::TrueColor, 0xce9178),
            Some(Color::Red)
        );
        assert_eq!(color("dark-ansi", Depth::TrueColor, 0xd4d4d4), None); // the terminal's own text colour
    }

    #[test]
    fn colours_come_down_to_the_terminals_depth() {
        assert_eq!(nearest_256(0x000000), 16);
        assert_eq!(nearest_256(0xffffff), 231);
        assert_eq!(nearest_256(0x808080), 244);
        assert_eq!(nearest_256(0xff0000), 196);
        assert_eq!(
            color("dark", Depth::Ansi256, 0x1e1e1e),
            Some(Color::Indexed(234))
        );
        assert_eq!(nearest_16(0xf85149), Color::LightRed);
        assert_eq!(color("dark", Depth::Ansi16, 0x3fb950), Some(Color::Green));
        assert_eq!(nearest_16(0x569cd6), Color::LightBlue);
        assert_eq!(nearest_16(0xd4d4d4), Color::Gray);
        assert_eq!(nearest_16(0x1e1e1e), Color::Black);
        assert_eq!(nearest_16(0xd29922), Color::LightYellow);
    }

    #[test]
    fn every_theme_maps_every_mask_colour() {
        for theme in THEMES {
            for (hex, _) in DARK {
                let h = u32::from_str_radix(&hex[1..], 16).unwrap();
                let _ = color(theme, Depth::Ansi256, h);
            }
        }
    }
}
