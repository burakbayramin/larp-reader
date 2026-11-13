//! Mask style strings (`"bold #f85149"`, `"book #d4d4d4 on #1f3d27"`) as terminal styles.

use ratatui::style::{Color, Modifier, Style};

use super::theme;

/// A `#rrggbb` colour in `theme` at `depth`.
fn hex(s: &str, theme_name: &str, depth: theme::Depth) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    theme::color(theme_name, depth, v)
}

/// The terminal style for a mask style string in the current theme.
pub fn style(spec: &str) -> Style {
    let (name, depth) = theme::current();
    style_in(spec, &name, depth)
}

/// Default text and background of the dark palette (the "text" and "background" roles).
const BASE: &str = "#d4d4d4 on #1e1e1e";

/// The screen's base style (background and text) in the current theme.
pub fn base() -> Style {
    let (name, depth) = theme::current();
    base_in(&name, depth)
}

/// The screen's base style in `theme_name` at `depth`; the ansi themes leave the terminal's own.
pub fn base_in(theme_name: &str, depth: theme::Depth) -> Style {
    style_in(BASE, theme_name, depth)
}

/// The terminal style for a mask style string; `book` (the internal text marker) never shows.
pub fn style_in(spec: &str, theme_name: &str, depth: theme::Depth) -> Style {
    let mut out = Style::default();
    let mut tokens = spec.split_whitespace();
    while let Some(t) = tokens.next() {
        match t {
            "bold" => out = out.add_modifier(Modifier::BOLD),
            "reverse" => out = out.add_modifier(Modifier::REVERSED),
            "on" => {
                if let Some(c) = tokens.next().and_then(|t| hex(t, theme_name, depth)) {
                    out = out.bg(c);
                }
            }
            _ => {
                if let Some(c) = hex(t, theme_name, depth) {
                    out = out.fg(c);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::theme::Depth;

    #[test]
    fn colours_backgrounds_and_modifiers() {
        let s = style_in(
            "book bold #f85149 on #1f3d27 reverse",
            "dark",
            Depth::TrueColor,
        );
        assert_eq!(s.fg, Some(Color::Rgb(0xf8, 0x51, 0x49)));
        assert_eq!(s.bg, Some(Color::Rgb(0x1f, 0x3d, 0x27)));
        assert!(s.add_modifier.contains(Modifier::BOLD | Modifier::REVERSED));
        assert_eq!(style_in("book", "dark", Depth::TrueColor), Style::default());
        // Apple's Terminal gets palette colours it can show
        assert_eq!(
            style_in("#f85149", "dark", Depth::Ansi256).fg,
            Some(Color::Indexed(203))
        );
        assert_eq!(
            style_in("#f85149", "light", Depth::TrueColor).fg,
            Some(Color::Rgb(0xcf, 0x22, 0x2e))
        );
    }

    #[test]
    fn base_style_paints_background_per_theme_and_depth() {
        let dark = base_in("dark", Depth::TrueColor);
        assert_eq!(dark.bg, Some(Color::Rgb(0x1e, 0x1e, 0x1e)));
        assert_eq!(dark.fg, Some(Color::Rgb(0xd4, 0xd4, 0xd4)));
        let light = base_in("light", Depth::TrueColor);
        assert_eq!(light.bg, Some(Color::Rgb(0xff, 0xff, 0xff)));
        assert_eq!(light.fg, Some(Color::Rgb(0x1f, 0x23, 0x28)));
        for t in ["dark-daltonized", "light-daltonized"] {
            assert!(base_in(t, Depth::TrueColor).bg.is_some(), "{t}");
        }
        for t in ["dark-ansi", "light-ansi"] {
            assert_eq!(base_in(t, Depth::TrueColor), Style::default(), "{t}");
        }
        assert!(matches!(
            base_in("light", Depth::Ansi256).bg,
            Some(Color::Indexed(_))
        ));
    }
}
