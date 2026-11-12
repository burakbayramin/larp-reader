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
}
