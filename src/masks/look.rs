//! How a piece of mask output looks: a colour role (or a literal colour) for the
//! text and the background, bold / reversed, and whether it carries text of the book.
//!
//! Masks speak in the roles of the dark theme (VS Code Dark+ code colours, GitHub
//! terminal colours). The UI resolves a `Look` against a theme and the terminal's
//! colour depth; masks never see either.

/// A colour by what it means; `dark` gives its value in the dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Background,
    Text,
    Gutter,
    Comment,
    Keyword,
    /// Control-flow keywords (`if`, `return`...).
    Control,
    Function,
    Type,
    String,
    Number,
    Variable,
    Constant,
    Green,
    Yellow,
    Red,
    Blue,
    Cyan,
    Dim,
    AddedBackground,
    RemovedBackground,
    PanelBackground,
    PanelText,
    PanelBorder,
}

impl Role {
    pub const ALL: [Role; 23] = [
        Role::Background,
        Role::Text,
        Role::Gutter,
        Role::Comment,
        Role::Keyword,
        Role::Control,
        Role::Function,
        Role::Type,
        Role::String,
        Role::Number,
        Role::Variable,
        Role::Constant,
        Role::Green,
        Role::Yellow,
        Role::Red,
        Role::Blue,
        Role::Cyan,
        Role::Dim,
        Role::AddedBackground,
        Role::RemovedBackground,
        Role::PanelBackground,
        Role::PanelText,
        Role::PanelBorder,
    ];

    /// The colour in the dark theme, `0xrrggbb`.
    pub const fn dark(self) -> u32 {
        match self {
            Role::Background => 0x1e1e1e,
            Role::Text => 0xd4d4d4,
            Role::Gutter => 0x858585,
            Role::Comment => 0x6a9955,
            Role::Keyword => 0x569cd6,
            Role::Control => 0xc586c0,
            Role::Function => 0xdcdcaa,
            Role::Type => 0x4ec9b0,
            Role::String => 0xce9178,
            Role::Number => 0xb5cea8,
            Role::Variable => 0x9cdcfe,
            Role::Constant => 0x4fc1ff,
            Role::Green => 0x3fb950,
            Role::Yellow => 0xd29922,
            Role::Red => 0xf85149,
            Role::Blue => 0x58a6ff,
            Role::Cyan => 0x39c5cf,
            Role::Dim => 0x6e7681,
            Role::AddedBackground => 0x1f3d27,
            Role::RemovedBackground => 0x4b1f22,
            Role::PanelBackground => 0x252526,
            Role::PanelText => 0xcccccc,
            Role::PanelBorder => 0x3c3c3c,
        }
    }
}

/// A colour: a theme role, or a literal `0xrrggbb` every theme shows as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ink {
    Role(Role),
    Rgb(u32),
}

/// The look of a segment. `Look::default()` is the terminal's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Look {
    pub fg: Option<Ink>,
    pub bg: Option<Ink>,
    pub bold: bool,
    pub reverse: bool,
    /// The segment carries text of the book (counted, searched, typed out, hidden).
    pub book: bool,
}

impl Look {
    pub const PLAIN: Look = Look {
        fg: None,
        bg: None,
        bold: false,
        reverse: false,
        book: false,
    };

    /// Text in the colour of `role`.
    pub const fn fg(role: Role) -> Look {
        Look {
            fg: Some(Ink::Role(role)),
            ..Look::PLAIN
        }
    }

    /// Text in a literal colour, the same in every theme.
    pub const fn rgb(hex: u32) -> Look {
        Look {
            fg: Some(Ink::Rgb(hex)),
            ..Look::PLAIN
        }
    }

    pub const fn bold(self) -> Look {
        Look { bold: true, ..self }
    }

    pub const fn reverse(self) -> Look {
        Look {
            reverse: true,
            ..self
        }
    }

    /// On the background colour of `role`.
    pub const fn on(self, role: Role) -> Look {
        Look {
            bg: Some(Ink::Role(role)),
            ..self
        }
    }

    /// Marked as text of the book.
    pub const fn book(self) -> Look {
        Look { book: true, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_set_one_thing_each() {
        let l = Look::fg(Role::Red).bold().on(Role::AddedBackground).book();
        assert_eq!(l.fg, Some(Ink::Role(Role::Red)));
        assert_eq!(l.bg, Some(Ink::Role(Role::AddedBackground)));
        assert!(l.bold && l.book && !l.reverse);
        assert_eq!(Look::default(), Look::PLAIN);
        assert!(Look::PLAIN.reverse().reverse);
    }

    #[test]
    fn roles_have_distinct_dark_colours() {
        let set: std::collections::HashSet<u32> = Role::ALL.iter().map(|r| r.dark()).collect();
        assert_eq!(set.len(), Role::ALL.len());
    }
}
