//! Remappable keys: the `keys` object of `config.json`.
//!
//! Each name in the object is an action and its value a key name or a list of
//! them; it replaces that action's default keys, the other actions keep theirs.
//! Unlike the rest of the config, a bad `keys` object is not quietly ignored:
//! `larpr build` refuses to start and says what is wrong, since a silently
//! dropped panic key would be worse than no start at all.
//!
//! ```json
//! "keys": { "panic": "F12", "bookmark": ["B", "ctrl+b"], "auto": [] }
//! ```

use std::collections::HashMap;
use std::fmt;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

/// Everything a key can be bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    Quit,
    PageDown,
    PageUp,
    LineDown,
    LineUp,
    Auto,
    Faster,
    Slower,
    NextChapter,
    PrevChapter,
    Search,
    FindNext,
    FindPrev,
    Bookmark,
    NextMark,
    PrevMark,
    NextMask,
    PrevMask,
    NextLang,
    PrevLang,
    MoreNoise,
    LessNoise,
    NextFile,
    PrevFile,
    Theme,
    Input,
    Stats,
    Menu,
    Panel,
    Panic,
}

/// Every action with its config name and default keys; the defaults are the
/// keys larpr always had.
pub const ACTIONS: &[(Action, &str, &[&str])] = &[
    (Action::Quit, "quit", &["q"]),
    (Action::PageDown, "page_down", &["space", "pagedown"]),
    (Action::PageUp, "page_up", &["u", "pageup"]),
    (Action::LineDown, "line_down", &["j", "down"]),
    (Action::LineUp, "line_up", &["k", "up"]),
    (Action::Auto, "auto", &["a"]),
    (Action::Faster, "faster", &["+"]),
    (Action::Slower, "slower", &["-"]),
    (Action::NextChapter, "next_chapter", &["]"]),
    (Action::PrevChapter, "prev_chapter", &["["]),
    (Action::Search, "search", &["/"]),
    (Action::FindNext, "find_next", &["f"]),
    (Action::FindPrev, "find_prev", &["F"]),
    (Action::Bookmark, "bookmark", &["b"]),
    (Action::NextMark, "next_mark", &["'"]),
    (Action::PrevMark, "prev_mark", &["\""]),
    (Action::NextMask, "next_view", &["m"]),
    (Action::PrevMask, "prev_view", &["M"]),
    (Action::NextLang, "next_lang", &["l"]),
    (Action::PrevLang, "prev_lang", &["L"]),
    (Action::MoreNoise, "more_noise", &["n"]),
    (Action::LessNoise, "less_noise", &["N"]),
    (Action::NextFile, "next_file", &["o"]),
    (Action::PrevFile, "prev_file", &["O"]),
    (Action::Theme, "theme", &["t"]),
    (Action::Input, "input", &["i"]),
    (Action::Stats, "stats", &["S"]),
    (Action::Menu, "menu", &[":"]),
    (Action::Panel, "panel", &["esc"]),
    (Action::Panic, "panic", &["`"]),
];

impl Action {
    pub fn name(self) -> &'static str {
        ACTIONS.iter().find(|(a, ..)| *a == self).expect("listed").1
    }

    fn defaults(self) -> &'static [&'static str] {
        ACTIONS.iter().find(|(a, ..)| *a == self).expect("listed").2
    }

    fn from_name(name: &str) -> Option<Action> {
        ACTIONS
            .iter()
            .find(|(_, n, _)| *n == name)
            .map(|(a, ..)| *a)
    }
}

/// One key as the config names it: a key code, with or without Ctrl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    code: KeyCode,
    ctrl: bool,
}

const NAMED: &[(&str, KeyCode)] = &[
    ("space", KeyCode::Char(' ')),
    ("esc", KeyCode::Esc),
    ("escape", KeyCode::Esc),
    ("enter", KeyCode::Enter),
    ("return", KeyCode::Enter),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("del", KeyCode::Delete),
    ("insert", KeyCode::Insert),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pgup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
    ("pgdn", KeyCode::PageDown),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
];

impl Key {
    /// `"q"`, `"F"` (letters are case sensitive), `"space"`, `"pagedown"`,
    /// `"f5"`, `"ctrl+b"`; named keys ignore case.
    pub fn parse(name: &str) -> Option<Key> {
        let (ctrl, rest) = match name.get(..5) {
            Some(p) if p.eq_ignore_ascii_case("ctrl+") && name.len() > 5 => (true, &name[5..]),
            _ => (false, name),
        };
        let mut chars = rest.chars();
        let code = match (chars.next(), chars.next()) {
            (Some(c), None) if !c.is_control() => {
                // the terminal reports Ctrl with a letter as the lower case letter
                KeyCode::Char(if ctrl { c.to_ascii_lowercase() } else { c })
            }
            _ => {
                let low = rest.to_ascii_lowercase();
                match NAMED.iter().find(|(n, _)| *n == low) {
                    Some((_, code)) => *code,
                    None => {
                        let n: u8 = low.strip_prefix('f')?.parse().ok()?;
                        if !(1..=12).contains(&n) {
                            return None;
                        }
                        KeyCode::F(n)
                    }
                }
            }
        };
        Some(Key { code, ctrl })
    }

    /// Keys larpr keeps for itself: the digits open listed files, Ctrl+Q always quits.
    fn reserved(self) -> Option<&'static str> {
        match self.code {
            KeyCode::Char('1'..='9') => Some("opens a listed input"),
            KeyCode::Char('q') if self.ctrl => Some("always quits"),
            _ => None,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("ctrl+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("space"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::PageUp => f.write_str("pgup"),
            KeyCode::PageDown => f.write_str("pgdn"),
            KeyCode::F(n) => write!(f, "f{n}"),
            code => {
                let name = NAMED.iter().find(|(_, c)| *c == code).map(|(n, _)| *n);
                f.write_str(name.unwrap_or("?"))
            }
        }
    }
}

/// The keys of every action, and the action of every key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    keys: HashMap<Action, Vec<Key>>,
    actions: HashMap<Key, Action>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap::parse(&Value::Null).expect("the defaults are valid")
    }
}

impl Keymap {
    /// The keymap from the config's `keys` value (`null` when there is none),
    /// or a message saying what is wrong with it.
    pub fn parse(value: &Value) -> Result<Keymap, String> {
        let overrides = match value {
            Value::Null => serde_json::Map::new(),
            Value::Object(o) => o.clone(),
            _ => {
                return Err("keys: expected an object of action names and keys, \
                            like {\"panic\": \"f12\"}"
                    .into());
            }
        };
        let mut keys: HashMap<Action, Vec<Key>> = HashMap::new();
        for (name, value) in &overrides {
            let action = Action::from_name(name).ok_or_else(|| {
                let known: Vec<&str> = ACTIONS.iter().map(|(_, n, _)| *n).collect();
                format!(
                    "keys: unknown action \"{name}\"; the actions are: {}",
                    known.join(", ")
                )
            })?;
            let names: Vec<&str> = match value {
                Value::String(s) => vec![s.as_str()],
                Value::Array(items) => items
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Option<_>>()
                    .ok_or_else(|| format!("keys.{name}: every key must be a string"))?,
                _ => {
                    return Err(format!(
                        "keys.{name}: expected a key name or a list of key names"
                    ));
                }
            };
            keys.insert(action, parse_list(name, &names)?);
        }
        for &(action, name, defaults) in ACTIONS {
            if let std::collections::hash_map::Entry::Vacant(slot) = keys.entry(action) {
                slot.insert(parse_list(name, defaults)?);
            }
        }
        if keys[&Action::Panic].is_empty() {
            return Err("keys.panic: the panic key can be changed but never removed".into());
        }

        let mut actions: HashMap<Key, Action> = HashMap::new();
        for &(action, ..) in ACTIONS {
            for &key in &keys[&action] {
                if let Some(other) = actions.insert(key, action) {
                    return Err(format!(
                        "keys: \"{key}\" is bound to both {} and {}; \
                         give one of them another key",
                        other.name(),
                        action.name()
                    ));
                }
            }
        }
        Ok(Keymap { keys, actions })
    }

    /// The action `key` triggers, if any. Shift is part of the character
    /// (`F`, `:`); a key pressed with Ctrl falls back to its plain binding
    /// unless a `ctrl+` binding claims it.
    pub fn action(&self, key: &KeyEvent) -> Option<Action> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let exact = Key {
            code: key.code,
            ctrl,
        };
        self.actions.get(&exact).copied().or_else(|| {
            ctrl.then(|| {
                self.actions.get(&Key {
                    code: key.code,
                    ctrl: false,
                })
            })
            .flatten()
            .copied()
        })
    }

    /// Whether `key` is a panic key: checked before anything else, everywhere.
    pub fn is_panic(&self, key: &KeyEvent) -> bool {
        self.action(key) == Some(Action::Panic)
    }

    /// The keys bound to `action`, in the order the config lists them.
    pub fn keys(&self, action: Action) -> &[Key] {
        &self.keys[&action]
    }

    /// Whether `action` still has its default keys.
    pub fn is_default(&self, action: Action) -> bool {
        let defaults: Vec<Key> = action
            .defaults()
            .iter()
            .filter_map(|n| Key::parse(n))
            .collect();
        self.keys[&action] == defaults
    }
}

fn parse_list(action: &str, names: &[&str]) -> Result<Vec<Key>, String> {
    let mut keys: Vec<Key> = Vec::new();
    for name in names {
        let key = Key::parse(name).ok_or_else(|| {
            format!(
                "keys.{action}: unknown key \"{name}\"; use a single character, \
                 a name like space, esc, enter, tab, up, pgdn, f1-f12, or ctrl+<character>"
            )
        })?;
        if let Some(why) = key.reserved() {
            return Err(format!("keys.{action}: \"{key}\" is reserved (it {why})"));
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn err(value: Value) -> String {
        Keymap::parse(&value).unwrap_err()
    }

    #[test]
    fn defaults_are_the_old_keys() {
        let k = Keymap::default();
        let cases = [
            (KeyCode::Char(' '), Action::PageDown),
            (KeyCode::PageDown, Action::PageDown),
            (KeyCode::Char('u'), Action::PageUp),
            (KeyCode::Down, Action::LineDown),
            (KeyCode::Char('F'), Action::FindPrev),
            (KeyCode::Char('"'), Action::PrevMark),
            (KeyCode::Esc, Action::Panel),
            (KeyCode::Char('`'), Action::Panic),
            (KeyCode::Char('q'), Action::Quit),
        ];
        for (code, action) in cases {
            assert_eq!(k.action(&press(code)), Some(action), "{code:?}");
        }
        assert_eq!(k.action(&press(KeyCode::Char('z'))), None);
        assert_eq!(k.action(&press(KeyCode::Char('1'))), None);
        for &(action, ..) in ACTIONS {
            assert!(k.is_default(action), "{action:?}");
        }
    }

    #[test]
    fn every_action_has_a_unique_name_and_default() {
        let mut names: Vec<&str> = ACTIONS.iter().map(|(_, n, _)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ACTIONS.len());
    }

    #[test]
    fn an_override_replaces_only_that_actions_keys() {
        let k = Keymap::parse(&json!({"bookmark": ["B", "ctrl+B"], "panic": "F12"})).unwrap();
        assert_eq!(k.action(&press(KeyCode::Char('B'))), Some(Action::Bookmark));
        assert_eq!(
            k.action(&KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            Some(Action::Bookmark)
        );
        assert_eq!(k.action(&press(KeyCode::Char('b'))), None);
        assert!(k.is_panic(&press(KeyCode::F(12))));
        assert!(!k.is_panic(&press(KeyCode::Char('`'))));
        assert!(!k.is_default(Action::Bookmark));
        assert!(k.is_default(Action::Quit));
        assert_eq!(
            k.keys(Action::Bookmark)
                .iter()
                .map(Key::to_string)
                .collect::<Vec<_>>(),
            ["B", "ctrl+b"]
        );
    }

    #[test]
    fn ctrl_falls_back_to_the_plain_key() {
        let k = Keymap::default();
        let ctrl_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
        assert_eq!(k.action(&ctrl_j), Some(Action::LineDown));
    }

    #[test]
    fn an_action_can_be_unbound_but_not_panic() {
        let k = Keymap::parse(&json!({"auto": []})).unwrap();
        assert_eq!(k.action(&press(KeyCode::Char('a'))), None);
        let e = err(json!({"panic": []}));
        assert!(e.contains("panic") && e.contains("never removed"), "{e}");
    }

    #[test]
    fn unknown_actions_and_keys_are_named() {
        let e = err(json!({"jump": "x"}));
        assert!(e.contains("unknown action \"jump\""), "{e}");
        assert!(e.contains("bookmark"), "lists the known ones: {e}");
        let e = err(json!({"bookmark": "hyper+x"}));
        assert!(
            e.starts_with("keys.bookmark: unknown key \"hyper+x\""),
            "{e}"
        );
        assert!(err(json!({"bookmark": "f13"})).contains("unknown key"));
        assert!(err(json!({"bookmark": 5})).contains("expected a key name"));
        assert!(err(json!({"bookmark": ["x", 5]})).contains("must be a string"));
        assert!(err(json!(["x"])).starts_with("keys: expected an object"));
    }

    #[test]
    fn conflicts_name_both_actions() {
        let e = err(json!({"bookmark": "m"}));
        assert!(
            e.contains("\"m\"") && e.contains("bookmark") && e.contains("next_view"),
            "{e}"
        );
        // moving the other action away first resolves it
        Keymap::parse(&json!({"bookmark": "m", "next_view": "v"})).unwrap();
        let e = err(json!({"panic": "q"}));
        assert!(e.contains("quit") && e.contains("panic"), "{e}");
    }

    #[test]
    fn reserved_keys_are_refused() {
        assert!(err(json!({"bookmark": "3"})).contains("reserved"));
        assert!(err(json!({"panic": "ctrl+q"})).contains("reserved"));
    }

    #[test]
    fn key_names() {
        let ok = [
            ("space", "space"),
            ("SPACE", "space"),
            ("Esc", "esc"),
            ("pagedown", "pgdn"),
            ("f5", "f5"),
            ("F", "F"),
            ("ctrl+X", "ctrl+x"),
            ("`", "`"),
            ("+", "+"),
        ];
        for (name, shown) in ok {
            assert_eq!(
                Key::parse(name).map(|k| k.to_string()).as_deref(),
                Some(shown),
                "{name}"
            );
        }
        for bad in ["", "ctrl+", "f0", "nope", "ab"] {
            assert_eq!(Key::parse(bad), None, "{bad}");
        }
    }
}
