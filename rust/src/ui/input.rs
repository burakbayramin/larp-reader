//! A one-line text field for the bottom bars (search, fake shell, path in the menu).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LineInput {
    pub value: String,
}

/// What a key did to the field.
#[derive(Debug, PartialEq, Eq)]
pub enum InputEvent {
    /// The value changed or the key was taken.
    Edited,
    /// Enter: the value is done.
    Submit(String),
    /// A key the field does not handle (Esc, arrows, the hide key …).
    Ignored,
}

impl LineInput {
    pub fn handle(&mut self, key: KeyEvent) -> InputEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => InputEvent::Submit(std::mem::take(&mut self.value)),
            KeyCode::Backspace => {
                self.value.pop();
                InputEvent::Edited
            }
            KeyCode::Char('u') if ctrl => {
                self.value.clear();
                InputEvent::Edited
            }
            KeyCode::Char('`') => InputEvent::Ignored,
            KeyCode::Char(c) if !ctrl => {
                self.value.push(c);
                InputEvent::Edited
            }
            _ => InputEvent::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_editing_and_submitting() {
        let mut f = LineInput::default();
        for c in "lsx".chars() {
            assert_eq!(f.handle(k(KeyCode::Char(c))), InputEvent::Edited);
        }
        f.handle(k(KeyCode::Backspace));
        assert_eq!(f.handle(k(KeyCode::Enter)), InputEvent::Submit("ls".into()));
        assert_eq!(f.value, "");
        assert_eq!(f.handle(k(KeyCode::Esc)), InputEvent::Ignored);
        assert_eq!(f.handle(k(KeyCode::Char('`'))), InputEvent::Ignored);
    }
}
