//! Mask style strings (`"bold #f85149"`, `"book #d4d4d4 on #1f3d27"`) as terminal styles.

use ratatui::style::{Color, Modifier, Style};

fn hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The terminal style for a mask style string; `book` (the internal text marker) never shows.
pub fn style(spec: &str) -> Style {
    let mut out = Style::default();
    let mut tokens = spec.split_whitespace();
    while let Some(t) = tokens.next() {
        match t {
            "bold" => out = out.add_modifier(Modifier::BOLD),
            "reverse" => out = out.add_modifier(Modifier::REVERSED),
            "on" => {
                if let Some(c) = tokens.next().and_then(hex) {
                    out = out.bg(c);
                }
            }
            _ => {
                if let Some(c) = hex(t) {
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

    #[test]
    fn colours_backgrounds_and_modifiers() {
        let s = style("book bold #f85149 on #1f3d27 reverse");
        assert_eq!(s.fg, Some(Color::Rgb(0xf8, 0x51, 0x49)));
        assert_eq!(s.bg, Some(Color::Rgb(0x1f, 0x3d, 0x27)));
        assert!(s.add_modifier.contains(Modifier::BOLD | Modifier::REVERSED));
        assert_eq!(style("book"), Style::default());
    }
}
