//! Colour themes and what the terminal can show.
//!
//! Masks speak in colour roles (`masks::Role`), valued in the dark theme
//! (VS Code Dark+). A `Theme` maps each role to its own colour when drawing,
//! and the result is brought down to the terminal's depth: 24-bit where it is
//! supported, else the nearest of the 256 xterm colours (Apple's Terminal
//! before macOS 26) or of the 16 ANSI ones. The app owns the theme in use and
//! passes it to whatever draws.

use ratatui::style::{Color, Modifier, Style};

use crate::masks::{Ink, Look, Role};

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

/// A colour in a theme: a 24-bit value, a terminal palette colour, or the terminal's own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paint {
    Hex(u32),
    Named(Color),
    Default,
}

use Paint::{Default as Term, Hex, Named};

/// Light: VS Code Light+ code colours with GitHub's terminal and diff colours.
fn light(role: Role) -> Paint {
    Hex(match role {
        Role::Background => 0xffffff,
        Role::Text | Role::PanelText => 0x1f2328,
        Role::Gutter | Role::Dim => 0x6e7781,
        Role::Comment => 0x008000,
        Role::Keyword => 0x0000ff,
        Role::Control => 0xaf00db,
        Role::Function => 0x795e26,
        Role::Type => 0x267f99,
        Role::String => 0xa31515,
        Role::Number => 0x098658,
        Role::Variable => 0x001080,
        Role::Constant => 0x0070c1,
        Role::Green => 0x1a7f37,
        Role::Yellow => 0x9a6700,
        Role::Red => 0xcf222e,
        Role::Blue => 0x0969da,
        Role::Cyan => 0x1b7c83,
        Role::AddedBackground => 0xdafbe1,
        Role::RemovedBackground => 0xffebe9,
        Role::PanelBackground => 0xf6f8fa,
        Role::PanelBorder => 0xd0d7de,
    })
}

/// Monokai on pure black, as a 256-colour terminal shows a coding session:
/// cyan declarations, pink control words, lime calls, beige strings, violet
/// numbers, deep green and red diff backgrounds.
fn monokai(role: Role) -> Paint {
    Hex(match role {
        Role::Background => 0x000000,
        Role::Text | Role::PanelText | Role::Type | Role::Variable => 0xf5f5f5,
        Role::Gutter | Role::PanelBorder => 0x6f6f6f,
        Role::Dim | Role::Comment => 0x878787,
        Role::Keyword => 0x5fd7ff,
        Role::Control => 0xff005f,
        Role::Function => 0xafd700,
        Role::String => 0xd7d787,
        Role::Number | Role::Constant => 0xaf87ff,
        Role::Green => 0x87d787,
        Role::Yellow => 0xffd700,
        Role::Red => 0xd75f5f,
        Role::Blue => 0x5f5fff,
        Role::Cyan => 0x5fd7d7,
        Role::AddedBackground => 0x005f00,
        Role::RemovedBackground => 0x5f0000,
        Role::PanelBackground => 0x1c1c1c,
    })
}

/// Colour-blind friendly: blue for added / passing, orange for removed / failing.
fn daltonized(role: Role, dark: bool) -> Option<Paint> {
    Some(Hex(match (role, dark) {
        (Role::Green, true) => 0x58a6ff,
        (Role::Red, true) => 0xf0883e,
        (Role::AddedBackground, true) => 0x12304a,
        (Role::RemovedBackground, true) => 0x4a2a14,
        (Role::Comment, true) => 0x8b949e,
        (Role::Green, false) => 0x0969da,
        (Role::Red, false) => 0xbc4c00,
        (Role::AddedBackground, false) => 0xddf4ff,
        (Role::RemovedBackground, false) => 0xfff1e5,
        (Role::Comment, false) => 0x57606a,
        _ => return None,
    }))
}

/// Only the terminal's 16 colours, so its own palette decides the shades.
fn ansi(role: Role, dark: bool) -> Paint {
    let bright = |c: Color, dim: Color| Named(if dark { c } else { dim });
    match role {
        Role::Background | Role::Text | Role::PanelText | Role::PanelBackground => Term,
        Role::Gutter | Role::Dim | Role::PanelBorder => Named(Color::DarkGray),
        Role::Comment | Role::Green => Named(Color::Green),
        Role::Keyword | Role::Blue => Named(Color::Blue),
        Role::Control => Named(Color::Magenta),
        Role::Function | Role::Yellow => Named(Color::Yellow),
        Role::Type | Role::Cyan => Named(Color::Cyan),
        Role::String => bright(Color::LightRed, Color::Red),
        Role::Number => bright(Color::LightGreen, Color::Green),
        Role::Variable => bright(Color::LightBlue, Color::Blue),
        Role::Constant => bright(Color::LightCyan, Color::Cyan),
        Role::Red => Named(Color::Red),
        Role::AddedBackground => Named(Color::Green),
        Role::RemovedBackground => Named(Color::Red),
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

/// The default text on the default background.
const BASE: Look = Look::fg(Role::Text).on(Role::Background);

/// A colour theme (one of `THEMES`) at the terminal's colour depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    name: &'static str,
    depth: Depth,
}

impl Default for Theme {
    /// Monokai, 24-bit.
    fn default() -> Theme {
        Theme::new("monokai", Depth::TrueColor)
    }
}

impl Theme {
    /// The theme called `name` (monokai if there is none such) at `depth`.
    pub fn new(name: &str, depth: Depth) -> Theme {
        let name = THEMES
            .iter()
            .find(|t| **t == name)
            .copied()
            .unwrap_or("monokai");
        Theme { name, depth }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn depth(&self) -> Depth {
        self.depth
    }

    /// What `role` is painted with in this theme.
    fn paint(&self, role: Role) -> Paint {
        let dark = Hex(role.dark());
        match self.name {
            "monokai" => monokai(role),
            "light" => light(role),
            "dark-daltonized" => daltonized(role, true).unwrap_or(dark),
            "light-daltonized" => daltonized(role, false).unwrap_or_else(|| light(role)),
            "dark-ansi" => ansi(role, true),
            "light-ansi" => ansi(role, false),
            _ => dark,
        }
    }

    /// A colour as the terminal should get it; None leaves the terminal's own.
    pub fn color(&self, ink: Ink) -> Option<Color> {
        let paint = match ink {
            Ink::Role(r) => self.paint(r),
            Ink::Rgb(h) => Hex(h),
        };
        match paint {
            Term => None,
            Named(c) => Some(c),
            Hex(h) => Some(match self.depth {
                Depth::TrueColor => {
                    let (r, g, b) = rgb(h);
                    Color::Rgb(r as u8, g as u8, b as u8)
                }
                Depth::Ansi256 => Color::Indexed(nearest_256(h)),
                Depth::Ansi16 => nearest_16(h),
            }),
        }
    }

    /// The terminal style for a look; `book` (the internal text marker) never shows.
    pub fn style(&self, look: Look) -> Style {
        let mut out = Style::default();
        if look.bold {
            out = out.add_modifier(Modifier::BOLD);
        }
        if look.reverse {
            out = out.add_modifier(Modifier::REVERSED);
        }
        if let Some(c) = look.fg.and_then(|i| self.color(i)) {
            out = out.fg(c);
        }
        if let Some(c) = look.bg.and_then(|i| self.color(i)) {
            out = out.bg(c);
        }
        out
    }

    /// The screen's base style (background and text); the ansi themes leave the terminal's own.
    pub fn base(&self) -> Style {
        self.style(BASE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(theme: &str, depth: Depth, role: Role) -> Option<Color> {
        Theme::new(theme, depth).color(Ink::Role(role))
    }

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
    fn monokai_lands_on_the_256_colours_of_a_terminal_coding_session() {
        let at = |role| color("monokai", Depth::Ansi256, role);
        let idx = |i| Some(Color::Indexed(i));
        assert_eq!(at(Role::Background), idx(16));
        assert_eq!(at(Role::Keyword), idx(81));
        assert_eq!(at(Role::Control), idx(197));
        assert_eq!(at(Role::Function), idx(148));
        assert_eq!(at(Role::String), idx(186));
        assert_eq!(at(Role::Number), idx(141));
        assert_eq!(at(Role::AddedBackground), idx(22));
        assert_eq!(at(Role::RemovedBackground), idx(52));
    }

    #[test]
    fn unknown_names_fall_back_to_monokai() {
        assert_eq!(Theme::new("nope", Depth::Ansi16).name(), "monokai");
        assert_eq!(Theme::new("nope", Depth::Ansi16).depth(), Depth::Ansi16);
        assert_eq!(Theme::new("light", Depth::TrueColor).name(), "light");
        assert_eq!(Theme::default(), Theme::new("monokai", Depth::TrueColor));
    }

    #[test]
    fn dark_keeps_its_colours_and_light_swaps_them() {
        assert_eq!(
            color("dark", Depth::TrueColor, Role::Comment),
            Some(Color::Rgb(0x6a, 0x99, 0x55))
        );
        assert_eq!(
            color("light", Depth::TrueColor, Role::Comment),
            Some(Color::Rgb(0x00, 0x80, 0x00))
        );
        assert_eq!(
            Theme::new("light", Depth::TrueColor).color(Ink::Rgb(0x123456)),
            Some(Color::Rgb(0x12, 0x34, 0x56))
        ); // not a theme colour
    }

    #[test]
    fn daltonized_themes_avoid_red_against_green() {
        for theme in ["dark-daltonized", "light-daltonized"] {
            let (added, removed) = (
                color(theme, Depth::TrueColor, Role::Green),
                color(theme, Depth::TrueColor, Role::Red),
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
            color("dark-ansi", Depth::TrueColor, Role::Green),
            Some(Color::Green)
        );
        assert_eq!(
            color("light-ansi", Depth::TrueColor, Role::String),
            Some(Color::Red)
        );
        assert_eq!(color("dark-ansi", Depth::TrueColor, Role::Text), None); // the terminal's own text colour
    }

    #[test]
    fn colours_come_down_to_the_terminals_depth() {
        assert_eq!(nearest_256(0x000000), 16);
        assert_eq!(nearest_256(0xffffff), 231);
        assert_eq!(nearest_256(0x808080), 244);
        assert_eq!(nearest_256(0xff0000), 196);
        assert_eq!(
            color("dark", Depth::Ansi256, Role::Background),
            Some(Color::Indexed(234))
        );
        assert_eq!(nearest_16(0xf85149), Color::LightRed);
        assert_eq!(
            color("dark", Depth::Ansi16, Role::Green),
            Some(Color::Green)
        );
        assert_eq!(nearest_16(0x569cd6), Color::LightBlue);
        assert_eq!(nearest_16(0xd4d4d4), Color::Gray);
        assert_eq!(nearest_16(0x1e1e1e), Color::Black);
        assert_eq!(nearest_16(0xd29922), Color::LightYellow);
    }

    #[test]
    fn looks_become_colours_backgrounds_and_modifiers() {
        let look = Look::fg(Role::Red)
            .bold()
            .on(Role::AddedBackground)
            .reverse()
            .book();
        let dark = Theme::new("dark", Depth::TrueColor);
        let s = dark.style(look);
        assert_eq!(s.fg, Some(Color::Rgb(0xf8, 0x51, 0x49)));
        assert_eq!(s.bg, Some(Color::Rgb(0x1f, 0x3d, 0x27)));
        assert!(s.add_modifier.contains(Modifier::BOLD | Modifier::REVERSED));
        assert_eq!(dark.style(Look::PLAIN.book()), Style::default());
        // Apple's Terminal gets palette colours it can show
        assert_eq!(
            Theme::new("dark", Depth::Ansi256)
                .style(Look::fg(Role::Red))
                .fg,
            Some(Color::Indexed(203))
        );
        assert_eq!(
            Theme::new("light", Depth::TrueColor)
                .style(Look::fg(Role::Red))
                .fg,
            Some(Color::Rgb(0xcf, 0x22, 0x2e))
        );
    }

    #[test]
    fn base_style_paints_background_per_theme_and_depth() {
        let base = |t: &str, d: Depth| Theme::new(t, d).base();
        let dark = base("dark", Depth::TrueColor);
        assert_eq!(dark.bg, Some(Color::Rgb(0x1e, 0x1e, 0x1e)));
        assert_eq!(dark.fg, Some(Color::Rgb(0xd4, 0xd4, 0xd4)));
        let light = base("light", Depth::TrueColor);
        assert_eq!(light.bg, Some(Color::Rgb(0xff, 0xff, 0xff)));
        assert_eq!(light.fg, Some(Color::Rgb(0x1f, 0x23, 0x28)));
        for t in ["dark-daltonized", "light-daltonized"] {
            assert!(base(t, Depth::TrueColor).bg.is_some(), "{t}");
        }
        for t in ["dark-ansi", "light-ansi"] {
            assert_eq!(base(t, Depth::TrueColor), Style::default(), "{t}");
        }
        assert!(matches!(
            base("light", Depth::Ansi256).bg,
            Some(Color::Indexed(_))
        ));
    }

    #[test]
    fn every_theme_maps_every_role_at_every_depth() {
        for theme in THEMES {
            for depth in [Depth::TrueColor, Depth::Ansi256, Depth::Ansi16] {
                let t = Theme::new(theme, depth);
                for role in Role::ALL {
                    let _ = t.color(Ink::Role(role));
                }
            }
        }
    }
}
