//! Mask looks as terminal styles.

use ratatui::style::{Color, Modifier, Style};

use super::theme;
use crate::masks::{Ink, Look, Role};

/// An ink in `theme` at `depth`.
fn ink(i: Ink, theme_name: &str, depth: theme::Depth) -> Option<Color> {
    let hex = match i {
        Ink::Role(r) => r.dark(),
        Ink::Rgb(h) => h,
    };
    theme::color(theme_name, depth, hex)
}

/// The terminal style for a look in the current theme.
pub fn style(look: Look) -> Style {
    let (name, depth) = theme::current();
    style_in(look, &name, depth)
}

/// Default text and background (the "text" and "background" roles).
const BASE: Look = Look::fg(Role::Text).on(Role::Background);

/// The screen's base style (background and text) in the current theme.
pub fn base() -> Style {
    let (name, depth) = theme::current();
    base_in(&name, depth)
}

/// The screen's base style in `theme_name` at `depth`; the ansi themes leave the terminal's own.
pub fn base_in(theme_name: &str, depth: theme::Depth) -> Style {
    style_in(BASE, theme_name, depth)
}

/// The terminal style for a look; `book` (the internal text marker) never shows.
pub fn style_in(look: Look, theme_name: &str, depth: theme::Depth) -> Style {
    let mut out = Style::default();
    if look.bold {
        out = out.add_modifier(Modifier::BOLD);
    }
    if look.reverse {
        out = out.add_modifier(Modifier::REVERSED);
    }
    if let Some(c) = look.fg.and_then(|i| ink(i, theme_name, depth)) {
        out = out.fg(c);
    }
    if let Some(c) = look.bg.and_then(|i| ink(i, theme_name, depth)) {
        out = out.bg(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::theme::Depth;

    #[test]
    fn colours_backgrounds_and_modifiers() {
        let look = Look::fg(Role::Red)
            .bold()
            .on(Role::AddedBackground)
            .reverse()
            .book();
        let s = style_in(look, "dark", Depth::TrueColor);
        assert_eq!(s.fg, Some(Color::Rgb(0xf8, 0x51, 0x49)));
        assert_eq!(s.bg, Some(Color::Rgb(0x1f, 0x3d, 0x27)));
        assert!(s.add_modifier.contains(Modifier::BOLD | Modifier::REVERSED));
        assert_eq!(
            style_in(Look::PLAIN.book(), "dark", Depth::TrueColor),
            Style::default()
        );
        // Apple's Terminal gets palette colours it can show
        assert_eq!(
            style_in(Look::fg(Role::Red), "dark", Depth::Ansi256).fg,
            Some(Color::Indexed(203))
        );
        assert_eq!(
            style_in(Look::fg(Role::Red), "light", Depth::TrueColor).fg,
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
