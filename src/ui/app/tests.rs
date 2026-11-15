//! Test support for `App`: a harness that drives it without a terminal, and
//! the helpers the per-area test files share.

use super::*;
use crate::state::load_progress;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

pub fn make_txt(dir: &Path, n: usize) -> PathBuf {
    let p = dir.join("sample.txt");
    let paras: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "Paragraph number {i} {}",
                "lorem ipsum dolor sit amet ".repeat(8)
            )
        })
        .collect();
    std::fs::write(&p, paras.join("\n\n")).unwrap();
    p
}

pub fn cfg() -> Config {
    Config {
        mask: "code".into(),
        lang: "python".into(),
        noise: 0,
        ..Config::default()
    }
}

pub fn key(name: &str) -> KeyEvent {
    let (code, mods) = match name {
        "space" => (KeyCode::Char(' '), KeyModifiers::NONE),
        "down" => (KeyCode::Down, KeyModifiers::NONE),
        "up" => (KeyCode::Up, KeyModifiers::NONE),
        "pageup" => (KeyCode::PageUp, KeyModifiers::NONE),
        "pagedown" => (KeyCode::PageDown, KeyModifiers::NONE),
        "enter" => (KeyCode::Enter, KeyModifiers::NONE),
        "escape" => (KeyCode::Esc, KeyModifiers::NONE),
        "backspace" => (KeyCode::Backspace, KeyModifiers::NONE),
        "tab" => (KeyCode::Tab, KeyModifiers::NONE),
        "left" => (KeyCode::Left, KeyModifiers::NONE),
        "right" => (KeyCode::Right, KeyModifiers::NONE),
        n if n.starts_with("ctrl+") => (
            KeyCode::Char(n.chars().last().unwrap()),
            KeyModifiers::CONTROL,
        ),
        n => {
            let c = n.chars().next().unwrap();
            let mods = if c.is_uppercase() {
                KeyModifiers::SHIFT
            } else {
                KeyModifiers::NONE
            };
            (KeyCode::Char(c), mods)
        }
    };
    KeyEvent::new(code, mods)
}

pub struct Harness {
    pub app: App,
    pub now: Instant,
    pub _dir: tempfile::TempDir,
    pub dir: PathBuf,
}

impl Harness {
    pub fn new(
        path: Option<PathBuf>,
        config: Config,
        dir: tempfile::TempDir,
        cols: u16,
        rows: u16,
    ) -> Harness {
        let now = Instant::now();
        let st = dir.path().join("st");
        std::fs::create_dir_all(&st).unwrap();
        let dir_path = dir.path().to_path_buf();
        Harness {
            app: App::new(path, config, Theme::default(), st, cols, rows, now),
            now,
            _dir: dir,
            dir: dir_path,
        }
    }

    /// The usual: a 10-paragraph text in the code mask, no filler.
    pub fn sample(cols: u16, rows: u16) -> Harness {
        let d = tempfile::tempdir().unwrap();
        let p = make_txt(d.path(), 10);
        Harness::new(Some(p), cfg(), d, cols, rows)
    }

    pub fn press(&mut self, keys: &[&str]) {
        for k in keys {
            self.app.handle_key(key(k), self.now);
        }
    }

    pub fn advance(&mut self, by: Duration) {
        self.now += by;
        self.app.on_time(self.now);
    }

    pub fn visible(&self) -> Vec<Line> {
        self.app.feed.visible_lines().to_vec()
    }

    pub fn status(&self) -> String {
        self.app.status_text()
    }

    pub fn state_dir(&self) -> PathBuf {
        self.dir.join("st")
    }

    /// The screen as a terminal would show it.
    pub fn screen(&self) -> String {
        let mut t =
            Terminal::new(TestBackend::new(self.app.cols as u16, self.app.rows as u16)).unwrap();
        t.draw(|f| self.app.draw(f)).unwrap();
        let buf = t.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The screen's cells drawn in `theme`, 24-bit.
    pub fn cells(&self, theme: &str) -> ratatui::buffer::Buffer {
        let theme = Theme::new(theme, crate::ui::theme::Depth::TrueColor);
        let mut t =
            Terminal::new(TestBackend::new(self.app.cols as u16, self.app.rows as u16)).unwrap();
        t.draw(|f| self.app.draw_in(f, &theme)).unwrap();
        t.backend().buffer().clone()
    }

    pub fn session(&mut self) -> &mut Session {
        self.app.session.as_mut().expect("a book is open")
    }
}

// -- test_app_auto_panic.py (auto) and test_app_hardening.py --

fn sample_with(n: usize, config: Config, cols: u16, rows: u16) -> Harness {
    let d = tempfile::tempdir().unwrap();
    let p = make_txt(d.path(), n);
    Harness::new(Some(p), config, d, cols, rows)
}

// -- test_pacing.py --

fn paced(wpm: u32) -> Harness {
    sample_with(
        30,
        Config {
            mask: "log".into(),
            wpm,
            ..cfg()
        },
        80,
        24,
    )
}

// -- test_app_auto_panic.py (panic) and test_navigate.py (shell) --

/// Distinctive words of the text (4+ letters), lower-cased.
fn text_words(h: &mut Harness) -> std::collections::HashSet<String> {
    let book = h.session().book.clone();
    book.chapters
        .iter()
        .flat_map(|c| &c.paragraphs)
        .flat_map(|p| {
            p.text
                .split(|c: char| !c.is_alphabetic())
                .map(str::to_lowercase)
                .collect::<Vec<_>>()
        })
        .filter(|w| w.chars().count() >= 4)
        .collect()
}

fn shown(h: &Harness) -> String {
    h.app
        .feed
        .history_lines()
        .iter()
        .map(Line::plain)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn leaked(h: &Harness, words: &std::collections::HashSet<String>) -> Vec<String> {
    let text = shown(h) + " " + &h.screen().to_lowercase();
    let found: std::collections::HashSet<String> = text
        .split(|c: char| !c.is_alphabetic())
        .map(String::from)
        .collect();
    words
        .iter()
        .filter(|w| found.contains(*w))
        .cloned()
        .collect()
}

fn type_text(h: &mut Harness, text: &str) {
    for c in text.chars() {
        h.app
            .handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), h.now);
    }
}

// -- test_navigate.py (sections, search, marks) and test_menu_bookmarks.py (b) --

fn sections_harness() -> Harness {
    let d = tempfile::tempdir().unwrap();
    let mut parts = vec![];
    for c in 0..3 {
        parts.push(format!("# Kısım {c}"));
        parts.extend((0..4).map(|i| format!("chapter{c} para{i} {}", "lorem ipsum ".repeat(30))));
    }
    let p = d.path().join("parts.md");
    std::fs::write(&p, parts.join("\n\n")).unwrap();
    Harness::new(Some(p), cfg(), d, 80, 24)
}

fn shelf() -> Harness {
    let d = tempfile::tempdir().unwrap();
    let folder = d.path().join("shelf");
    std::fs::create_dir(&folder).unwrap();
    for name in ["alpha", "beta", "gamma"] {
        let paras: Vec<String> = (0..10)
            .map(|i| {
                format!(
                    "{name} paragraph {i} {}",
                    "lorem ipsum dolor sit amet ".repeat(8)
                )
            })
            .collect();
        std::fs::write(folder.join(format!("{name}.txt")), paras.join("\n\n")).unwrap();
    }
    std::fs::write(folder.join("cover.jpg"), b"\xff\xd8\xff").unwrap();
    std::fs::create_dir(folder.join("notes")).unwrap();
    Harness::new(Some(folder.join("beta.txt")), cfg(), d, 100, 30)
}

mod agent;
mod hiding;
mod manual;
mod menu;
mod navigate;
mod pacing;
mod panel;
mod report;
mod stats;
mod views;
