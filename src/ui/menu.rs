//! The settings screen (`ui/menu.py`): plain text that blends in with the terminal.
//!
//! Tab / Up / Down move between fields, Left / Right change a value, Enter
//! applies (or opens the typed path, or jumps to the chosen mark or section),
//! Esc closes and the hide key hides everything.

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line as TLine;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::input::{InputEvent, LineInput};
use crate::book::Pos;
use crate::masks::{LANGS, MASK_NAMES};
use crate::state::{Config, HIDE_STYLES, THEMES};
use crate::text::truncate_cells;

const WPMS: &[u32] = &[120, 160, 200, 240, 280, 320, 400, 500, 650, 800];
const NOISE_LEVELS: &[u32] = &[0, 1, 2, 3, 4, 5];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuResult {
    /// Still open.
    Stay,
    Close,
    Panic,
    Apply {
        mask: String,
        lang: String,
        wpm: u32,
        noise: u8,
        hide: String,
        theme: String,
    },
    Open(String),
    Jump(Pos),
}

/// One `name: < value >` line.
struct Choice {
    label: &'static str,
    options: Vec<String>,
    index: usize,
}

impl Choice {
    fn new<T: ToString + PartialEq + PartialOrd + Clone>(
        label: &'static str,
        options: &[T],
        current: T,
    ) -> Choice {
        let mut opts = options.to_vec();
        if !opts.contains(&current) {
            opts.push(current.clone());
            opts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
        let index = opts.iter().position(|o| *o == current).unwrap_or(0);
        Choice {
            label,
            options: opts.iter().map(ToString::to_string).collect(),
            index,
        }
    }

    fn value(&self) -> &str {
        &self.options[self.index]
    }

    fn cycle(&mut self, delta: isize) {
        let n = self.options.len() as isize;
        self.index = (self.index as isize + delta).rem_euclid(n) as usize;
    }
}

/// A list the user picks one entry from.
struct Pick {
    entries: Vec<(String, Pos)>,
    selected: usize,
}

/// Where the keyboard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Path,
    Choice(usize),
    Marks,
    Sections,
}

pub struct Menu {
    path: LineInput,
    choices: Vec<Choice>,
    marks: Pick,
    sections: Pick,
    focus: Focus,
}

pub fn section_label(n: usize, title: Option<&str>) -> String {
    let title = title
        .unwrap_or("-")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let label = format!("{n:>3} {title}");
    if label.width() <= 48 {
        label
    } else {
        format!("{}~", truncate_cells(&label, 47))
    }
}

impl Menu {
    /// The settings screen for `config`, with `mask` as the view now shown.
    pub fn new(
        mask: &str,
        config: &Config,
        marks: Vec<(String, Pos)>,
        sections: Vec<(String, Pos)>,
    ) -> Menu {
        let names = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Menu {
            path: LineInput::default(),
            choices: vec![
                Choice::new("target", &names(MASK_NAMES), mask.to_string()),
                Choice::new("lang", &names(LANGS), config.lang.clone()),
                Choice::new("rate", WPMS, config.wpm),
                Choice::new("verbosity", NOISE_LEVELS, config.noise as u32),
                Choice::new("on hide", &names(HIDE_STYLES), config.hide.clone()),
                Choice::new("theme", &names(THEMES), config.theme.clone()),
            ],
            marks: Pick {
                entries: marks,
                selected: 0,
            },
            sections: Pick {
                entries: sections,
                selected: 0,
            },
            focus: Focus::Path,
        }
    }

    /// Fields in focus order; lists only when there is something to pick.
    fn order(&self) -> Vec<Focus> {
        let mut v = vec![Focus::Path];
        v.extend((0..self.choices.len()).map(Focus::Choice));
        if !self.marks.entries.is_empty() {
            v.push(Focus::Marks);
        }
        if self.sections.entries.len() > 1 {
            v.push(Focus::Sections);
        }
        v
    }

    fn move_focus(&mut self, delta: isize) {
        let order = self.order();
        let i = order.iter().position(|f| *f == self.focus).unwrap_or(0) as isize;
        self.focus = order[(i + delta).rem_euclid(order.len() as isize) as usize];
    }

    fn pick_mut(&mut self) -> Option<&mut Pick> {
        match self.focus {
            Focus::Marks => Some(&mut self.marks),
            Focus::Sections => Some(&mut self.sections),
            _ => None,
        }
    }

    fn apply(&self) -> MenuResult {
        let v = |i: usize| self.choices[i].value().to_string();
        MenuResult::Apply {
            mask: v(0),
            lang: v(1),
            wpm: v(2).parse().unwrap_or(crate::pace::DEFAULT_WPM),
            noise: v(3).parse().unwrap_or(2),
            hide: v(4),
            theme: v(5),
        }
    }

    /// Focus a field by its label (`"target"`, `"rate"` …) or `"path"`, `"marks"`, `"sections"`.
    pub fn focus(&mut self, name: &str) {
        self.focus = match name {
            "path" => Focus::Path,
            "marks" => Focus::Marks,
            "sections" => Focus::Sections,
            label => Focus::Choice(
                self.choices
                    .iter()
                    .position(|c| c.label == label)
                    .expect("a field"),
            ),
        };
    }

    pub fn path_value(&self) -> &str {
        &self.path.value
    }

    pub fn sections(&self) -> &[(String, Pos)] {
        &self.sections.entries
    }

    pub fn handle(&mut self, key: KeyEvent) -> MenuResult {
        let shift_tab = key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT));
        match key.code {
            KeyCode::Char('`') => return MenuResult::Panic,
            KeyCode::Esc => return MenuResult::Close,
            _ if shift_tab => self.move_focus(-1),
            KeyCode::Tab => self.move_focus(1),
            _ => {}
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            return MenuResult::Stay;
        }
        if self.focus == Focus::Path {
            return match key.code {
                KeyCode::Up => {
                    self.move_focus(-1);
                    MenuResult::Stay
                }
                KeyCode::Down => {
                    self.move_focus(1);
                    MenuResult::Stay
                }
                _ => match self.path.handle(key) {
                    InputEvent::Submit(p) => {
                        let p = p.trim().trim_matches(['"', '\'']).to_string();
                        if p.is_empty() {
                            MenuResult::Stay
                        } else {
                            MenuResult::Open(p)
                        }
                    }
                    _ => MenuResult::Stay,
                },
            };
        }
        if let Some(pick) = self.pick_mut() {
            let last = pick.entries.len().saturating_sub(1);
            match key.code {
                KeyCode::Enter => return MenuResult::Jump(pick.entries[pick.selected].1),
                KeyCode::Down if pick.selected < last => pick.selected += 1,
                KeyCode::Up if pick.selected > 0 => pick.selected -= 1,
                KeyCode::Down => self.move_focus(1),
                KeyCode::Up => self.move_focus(-1),
                _ => {}
            }
            return MenuResult::Stay;
        }
        let Focus::Choice(i) = self.focus else {
            return MenuResult::Stay;
        };
        match key.code {
            KeyCode::Left => self.choices[i].cycle(-1),
            KeyCode::Right => self.choices[i].cycle(1),
            KeyCode::Up => self.move_focus(-1),
            KeyCode::Down => self.move_focus(1),
            KeyCode::Enter => return self.apply(),
            _ => {}
        }
        MenuResult::Stay
    }

    pub fn draw(&self, frame: &mut Frame) {
        self.draw_with(frame, super::style::base());
    }

    /// Draw on a screen painted with `base` (the theme's background and text).
    pub fn draw_with(&self, frame: &mut Frame, base: Style) {
        let focused = Style::default().add_modifier(Modifier::REVERSED);
        let line = |text: String, on: bool| {
            if on {
                TLine::styled(text, focused)
            } else {
                TLine::raw(text)
            }
        };
        let mut lines = vec![TLine::raw("fcc config")];
        let path = if self.path.value.is_empty() && self.focus != Focus::Path {
            "input: path".to_string()
        } else {
            format!("input: {}", self.path.value)
        };
        lines.push(line(path, self.focus == Focus::Path));
        for (i, c) in self.choices.iter().enumerate() {
            lines.push(line(
                format!("{}: < {} >", c.label, c.value()),
                self.focus == Focus::Choice(i),
            ));
        }
        let list = |lines: &mut Vec<TLine<'static>>,
                    title: &str,
                    empty: &str,
                    pick: &Pick,
                    here: bool,
                    show: bool| {
            if !show {
                lines.push(TLine::raw(empty.to_string()));
                return;
            }
            lines.push(TLine::raw(format!("{title}:")));
            for (k, (label, _)) in pick.entries.iter().enumerate().take(10) {
                lines.push(line(format!("  {label}"), here && k == pick.selected));
            }
        };
        list(
            &mut lines,
            "marks",
            "marks: none",
            &self.marks,
            self.focus == Focus::Marks,
            !self.marks.entries.is_empty(),
        );
        list(
            &mut lines,
            "sections",
            &format!("sections: {}", self.sections.entries.len()),
            &self.sections,
            self.focus == Focus::Sections,
            self.sections.entries.len() > 1,
        );
        frame.render_widget(Paragraph::new(lines).style(base), frame.area());
        if self.focus == Focus::Path {
            let x = 7 + unicode_width::UnicodeWidthStr::width(self.path.value.as_str()) as u16;
            frame.set_cursor_position((x.min(frame.area().width.saturating_sub(1)), 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_label_cuts_wide_titles_to_cells() {
        let label = section_label(7, Some(&"漢字かな交じり文🙂".repeat(5)));
        assert!(label.width() <= 48, "{label}");
        assert!(label.width() >= 46 && label.ends_with('~'), "{label}");
        assert_eq!(section_label(7, Some("Short")), "  7 Short");
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn menu() -> Menu {
        Menu::new(
            "code",
            &Config {
                wpm: 233,
                ..Config::default()
            },
            vec![("L12".into(), (0, 3))],
            vec![("  1 Bir".into(), (0, 0)), ("  2 İki".into(), (1, 0))],
        )
    }

    #[test]
    fn right_on_each_choice_then_enter_applies() {
        let mut m = menu();
        for name in ["target", "lang", "rate", "verbosity", "on hide", "theme"] {
            m.focus(name);
            m.handle(key(KeyCode::Right));
        }
        m.focus("target");
        assert_eq!(
            m.handle(key(KeyCode::Enter)),
            MenuResult::Apply {
                mask: "log".into(),
                lang: "typescript".into(),
                wpm: 240,
                noise: 3,
                hide: "shell".into(),
                theme: "light".into()
            }
        );
    }

    #[test]
    fn typed_path_opens_and_lists_jump() {
        let mut m = menu();
        for c in "\"C:/k/a.epub\"".chars() {
            m.handle(key(KeyCode::Char(c)));
        }
        assert_eq!(
            m.handle(key(KeyCode::Enter)),
            MenuResult::Open("C:/k/a.epub".into())
        );
        m.focus("sections");
        m.handle(key(KeyCode::Down));
        assert_eq!(m.handle(key(KeyCode::Enter)), MenuResult::Jump((1, 0)));
        m.focus("marks");
        assert_eq!(m.handle(key(KeyCode::Enter)), MenuResult::Jump((0, 3)));
    }

    #[test]
    fn escape_closes_and_backtick_hides() {
        assert_eq!(menu().handle(key(KeyCode::Esc)), MenuResult::Close);
        assert_eq!(menu().handle(key(KeyCode::Char('`'))), MenuResult::Panic);
    }

    #[test]
    fn tab_walks_every_field_and_wraps() {
        let mut m = menu();
        let fields = m.order().len();
        for _ in 0..fields {
            m.handle(key(KeyCode::Tab));
        }
        assert_eq!(m.focus, Focus::Path);
    }
}
