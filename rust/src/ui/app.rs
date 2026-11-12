//! The interactive reader (`ui/app.py`), as a state machine the terminal loop drives.
//!
//! Keys, timer deadlines and resizes come in through `handle_key`, `on_time`
//! and `resize`; `draw` paints the current state. Time is always passed in, so
//! tests can drive the app without a terminal and without waiting.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

use super::feed::Feed;
use super::style::style;
use crate::book::{Book, Pos};
use crate::errors::FccError;
use crate::loaders::load_book;
use crate::masks::base::{error_lines, plain_line};
use crate::masks::time::civil;
use crate::masks::{Line, Mask, get_mask, seeded};
use crate::session::Session;
use crate::state::{Config, Entry, last_entry, load_progress, save_progress};

pub const MIN_WIDTH: usize = 40;
pub const AUTOSAVE: Duration = Duration::from_secs(10);
pub const FLASH: Duration = Duration::from_secs(2);
const WARNING: &str = "warning: window too narrow";
const STATUS_FG: &str = "#858585";

/// The configured mask, or the code mask plus the problem if it can't be built.
pub fn resolve_mask(config: &Config) -> (Box<dyn Mask>, Option<FccError>) {
    match get_mask(&config.mask, &config.lang) {
        Ok(m) => (m, None),
        Err(e) => (
            get_mask("code", "python").expect("built in"),
            Some(FccError::Other(e)),
        ),
    }
}

/// A position that exists in `book`, else the start.
fn valid_pos(book: &Book, pos: Pos) -> Pos {
    if book.paragraph(pos).is_some() {
        pos
    } else {
        (0, 0)
    }
}

/// Local-time-free `2026-11-12T10:04:05` (UTC) for the progress file's `updated_at`.
fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

pub struct App {
    path: Option<PathBuf>,
    pub config: Config,
    state_dir: PathBuf,
    pub session: Option<Session>,
    /// The mask while no book is open (errors are drawn with it).
    spare_mask: Option<Box<dyn Mask>>,
    /// Error lines shown above the text until the next move.
    notice: Vec<Line>,
    entry_path: Option<String>,
    cols: usize,
    rows: usize,
    pub auto: bool,
    pub panic: bool,
    pub feed: Feed,
    pub bookmarks: Vec<[usize; 2]>,
    flash: Option<(String, Instant)>,
    /// Manual mode: text lines on screen now, and whether more text follows them.
    page_rows: usize,
    more_below: bool,
    next_autosave: Instant,
    save_error_shown: bool,
    quit: bool,
}

impl App {
    pub fn new(
        path: Option<PathBuf>,
        config: Config,
        state_dir: PathBuf,
        cols: u16,
        rows: u16,
        now: Instant,
    ) -> App {
        let (mask, mask_error) = resolve_mask(&config);
        let mut app = App {
            path,
            config,
            state_dir,
            session: None,
            spare_mask: Some(mask),
            notice: vec![],
            entry_path: None,
            cols: cols as usize,
            rows: rows as usize,
            auto: false,
            panic: false,
            feed: Feed::default(),
            bookmarks: vec![],
            flash: None,
            page_rows: 0,
            more_below: false,
            next_autosave: now + AUTOSAVE,
            save_error_shown: false,
            quit: false,
        };
        if let Some(e) = mask_error {
            app.notice.extend(error_lines(&e, app.width()));
        }
        if let Err(e) = app.open() {
            app.notice.extend(error_lines(&e, app.width()));
        }
        app.redraw();
        app
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    // -- opening -------------------------------------------------------------

    fn open(&mut self) -> Result<(), FccError> {
        let progress = load_progress(&self.state_dir);
        let path = match &self.path {
            Some(p) => p.clone(),
            None => match last_entry(&progress) {
                Some((_, e)) => PathBuf::from(&e.path),
                None => return Err(FccError::NoInput("no input files".into())),
            },
        };
        self.load(&path)
    }

    fn load(&mut self, path: &Path) -> Result<(), FccError> {
        let book = load_book(path)?; // fails before any state changes
        let progress = load_progress(&self.state_dir);
        let entry = progress.get(&book.id);
        let pos = entry
            .map(|e| valid_pos(&book, (e.chapter, e.paragraph)))
            .unwrap_or((0, 0));
        self.bookmarks = entry.map(|e| e.bookmarks.clone()).unwrap_or_default();
        self.entry_path = Some(
            std::path::absolute(path)
                .unwrap_or(path.to_path_buf())
                .to_string_lossy()
                .into_owned(),
        );
        let mask = match self.session.take() {
            Some(s) => {
                // keep the current disguise: rebuild it fresh for the new text
                get_mask(s.mask().name(), &self.config.lang)
                    .unwrap_or_else(|_| resolve_mask(&self.config).0)
            }
            None => self
                .spare_mask
                .take()
                .unwrap_or_else(|| resolve_mask(&self.config).0),
        };
        self.session = Some(Session::new(
            book,
            mask,
            self.config.noise,
            self.width(),
            pos,
        ));
        Ok(())
    }

    // -- drawing -------------------------------------------------------------

    fn width(&self) -> usize {
        self.cols.max(1)
    }

    fn view_height(&self) -> usize {
        self.rows.saturating_sub(1).max(1)
    }

    /// Lay the screen out again from the session: warning, notices, header and text.
    pub fn redraw(&mut self) {
        self.feed.set_height(self.view_height());
        if self.panic {
            return;
        }
        self.feed.set_bottom_anchored(self.auto);
        let width = self.width();
        let mut lines = vec![];
        if width < MIN_WIDTH {
            lines.push(plain_line(
                WARNING.chars().take(width).collect::<String>(),
                "yellow",
            ));
        }
        lines.extend(self.notice.iter().cloned());
        let view = self.view_height();
        if let Some(s) = self.session.as_mut() {
            let at_top = s.book.index_of(s.pos()) == 0 && (self.auto || s.line_offset == 0);
            if at_top {
                let id = s.book.id.clone();
                lines.extend(s.mask_mut().header(&mut seeded(&id), width));
            }
            if self.auto {
                lines.extend(s.paragraph_lines(s.pos()));
            } else {
                let rows = view.saturating_sub(lines.len());
                let text = s.window(rows + 1);
                self.more_below = text.len() > rows;
                self.page_rows = rows.min(text.len());
                lines.extend(text.into_iter().take(rows));
            }
        }
        self.feed.set_lines(lines);
    }

    pub fn status_text(&self) -> String {
        if let Some((msg, _)) = &self.flash {
            return msg.clone();
        }
        match &self.session {
            Some(s) => {
                let (done, total) = s.progress();
                s.mask().progress_label(done, total, self.width())
            }
            None => String::new(),
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let [feed, status] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
        frame.render_widget(&self.feed, feed);
        frame.render_widget(
            Paragraph::new(self.status_text()).style(style(STATUS_FG)),
            status,
        );
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols as usize;
        self.rows = rows as usize;
        let width = self.width();
        if let Some(s) = self.session.as_mut()
            && s.width() != width
        {
            s.set_width(width);
            let pos = s.pos();
            s.set_pos(pos); // re-wrapped: restart at the paragraph's top
        }
        self.redraw();
    }

    // -- keys ----------------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') if ctrl => self.quit_and_save(),
            KeyCode::Char('q') => self.quit_and_save(),
            KeyCode::Char(' ') | KeyCode::PageDown => self.page(),
            KeyCode::Char('u') | KeyCode::PageUp => self.page_back(),
            KeyCode::Char('j') | KeyCode::Down => self.scroll(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll(-1),
            _ => {}
        }
        let _ = now;
    }

    /// Move the manual view by `n` rendered lines; never past the last screen.
    fn scroll(&mut self, n: isize) {
        if self.auto || self.panic {
            return;
        }
        let Some(s) = self.session.as_mut() else {
            return;
        };
        if n > 0 && !self.more_below {
            return; // the end of the text is already on screen
        }
        self.notice.clear();
        if n != 0 {
            s.scroll(n);
        }
        self.redraw();
    }

    /// The next screen starts at the first line this one did not show.
    fn page(&mut self) {
        self.scroll(self.page_rows as isize);
    }

    /// The screen that ends just above this one (or the first screen).
    fn page_back(&mut self) {
        if self.auto || self.panic {
            return;
        }
        let rows = self.view_height() - usize::from(self.width() < MIN_WIDTH);
        let Some(s) = self.session.as_mut() else {
            return;
        };
        self.notice.clear();
        s.scroll(-(rows.max(1) as isize));
        self.redraw();
    }

    fn quit_and_save(&mut self) {
        self.try_save();
        self.quit = true;
    }

    // -- time ----------------------------------------------------------------

    /// The next moment `on_time` has something to do.
    pub fn next_deadline(&self) -> Instant {
        let mut at = self.next_autosave;
        if let Some((_, until)) = &self.flash {
            at = at.min(*until);
        }
        at
    }

    pub fn on_time(&mut self, now: Instant) {
        if let Some((_, until)) = &self.flash
            && now >= *until
        {
            self.flash = None;
        }
        if now >= self.next_autosave {
            self.try_save();
            self.next_autosave = now + AUTOSAVE;
        }
    }

    pub fn flash(&mut self, msg: &str, now: Instant) {
        self.flash = Some((msg.to_string(), now + FLASH));
    }

    // -- saving --------------------------------------------------------------

    fn save(&self) -> std::io::Result<()> {
        let (Some(s), Some(path)) = (&self.session, &self.entry_path) else {
            return Ok(());
        };
        let mut data = load_progress(&self.state_dir);
        data.insert(
            s.book.id.clone(),
            Entry {
                path: path.clone(),
                chapter: s.pos().0,
                paragraph: s.pos().1,
                bookmarks: self.bookmarks.clone(),
                updated_at: timestamp(),
            },
        );
        save_progress(&self.state_dir, &data)
    }

    fn try_save(&mut self) {
        if let Err(e) = self.save() {
            if self.save_error_shown {
                return;
            }
            self.save_error_shown = true;
            let lines = error_lines(
                &FccError::Other(format!("could not save state: {}", e.kind())),
                self.width(),
            );
            self.notice.extend(lines.iter().cloned());
            if self.auto && !self.panic {
                self.feed.append(lines);
            } else {
                self.redraw();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
                app: App::new(path, config, st, cols, rows, now),
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
            let mut t = Terminal::new(TestBackend::new(self.app.cols as u16, self.app.rows as u16))
                .unwrap();
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

        pub fn session(&mut self) -> &mut Session {
            self.app.session.as_mut().expect("a book is open")
        }
    }

    // -- test_app_manual.py --

    #[test]
    fn manual_space_advances_and_status_updates() {
        let mut h = Harness::sample(80, 24);
        assert_eq!(h.session().progress().0, 1);
        h.press(&["space"]);
        let done = h.session().progress().0;
        assert!(done > 1);
        assert!(
            h.status().contains(&format!(" {done}/10")),
            "{}",
            h.status()
        );
    }

    #[test]
    fn space_starts_next_screen_at_first_unseen_line() {
        let mut h = Harness::sample(80, 24);
        let header = get_mask("code", "python")
            .unwrap()
            .header(&mut seeded("x"), 80);
        let stream = h.session().window(1000);
        let mut want = header.clone();
        want.extend(stream[..23 - header.len()].iter().cloned());
        assert_eq!(h.visible(), want);
        h.press(&["space"]);
        assert_eq!(
            h.visible(),
            stream[23 - header.len()..46 - header.len()].to_vec()
        );
        let s = h.session();
        let (pos, off) = (s.pos(), s.line_offset);
        assert_eq!(s.paragraph_lines(pos)[off], stream[23 - header.len()]);
        assert_ne!(pos, (0, 0));
    }

    #[test]
    fn down_and_j_scroll_one_line_up_and_k_back() {
        let mut h = Harness::sample(80, 24);
        let stream = h.session().window(1000);
        h.press(&["down"]);
        assert_eq!(h.visible()[..23], stream[1..24]);
        h.press(&["j"]);
        assert_eq!(h.visible()[..23], stream[2..25]);
        h.press(&["down", "up"]);
        assert_eq!(h.visible(), stream[2..25].to_vec());
        h.press(&["k"]);
        assert_eq!(h.visible(), stream[1..24].to_vec());
    }

    #[test]
    fn q_saves_progress_and_reopening_resumes() {
        let mut h = Harness::sample(80, 24);
        h.press(&["space", "space"]);
        let pos = h.session().pos();
        assert_ne!(pos, (0, 0));
        let id = h.session().book.id.clone();
        h.press(&["q"]);
        assert!(h.app.should_quit());
        let saved = load_progress(&h.state_dir());
        assert_eq!((saved[&id].chapter, saved[&id].paragraph), pos);
        let again = App::new(
            Some(h.dir.join("sample.txt")),
            cfg(),
            h.state_dir(),
            80,
            24,
            h.now,
        );
        assert_eq!(again.session.as_ref().unwrap().pos(), pos);
    }

    #[test]
    fn ctrl_q_also_saves() {
        let mut h = Harness::sample(80, 24);
        h.press(&["space", "space", "ctrl+q"]);
        assert!(h.app.should_quit());
        let pos = h.session().pos();
        let id = h.session().book.id.clone();
        let saved = load_progress(&h.state_dir());
        assert_eq!((saved[&id].chapter, saved[&id].paragraph), pos);
    }

    #[test]
    fn resize_keeps_paragraph() {
        let mut h = Harness::sample(80, 24);
        h.press(&["down", "down"]);
        let pos = h.session().pos();
        h.app.resize(50, 24);
        assert_eq!(h.session().pos(), pos);
        assert_eq!(h.session().width(), 50);
        let lines = h.visible();
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|l| l.plain().chars().count() <= 50));
        assert!(lines.iter().any(|l| l.pos == Some(pos)));
    }

    #[test]
    fn narrow_terminal_shows_one_warning() {
        let h = Harness::sample(38, 24);
        let lines = h.visible();
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.plain().contains("warning"))
                .count(),
            1
        );
        assert!(lines.iter().all(|l| l.plain().chars().count() <= 38));
        assert!(
            !Harness::sample(80, 24)
                .visible()
                .iter()
                .any(|l| l.plain().contains("warning"))
        );
    }

    #[test]
    fn empty_input_shows_error_not_crash() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("empty.txt");
        std::fs::write(&p, "").unwrap();
        let mut h = Harness::new(Some(p), cfg(), d, 80, 24);
        assert!(h.app.session.is_none());
        assert!(
            h.visible()
                .iter()
                .any(|l| l.plain().contains("empty input"))
        );
        h.press(&["space", "down", "up", "u", "q"]); // must not crash
    }

    #[test]
    fn unknown_mask_falls_back_to_code_with_error() {
        let d = tempfile::tempdir().unwrap();
        let p = make_txt(d.path(), 10);
        let config = Config {
            mask: "nope".into(),
            lang: "rust".into(),
            noise: 0,
            ..Config::default()
        };
        let mut h = Harness::new(Some(p), config, d, 80, 24);
        assert_eq!(h.session().mask().name(), "code");
        assert!(
            h.app
                .feed
                .history_lines()
                .iter()
                .any(|l| l.plain().starts_with("error:"))
        );
        assert!(h.visible().iter().any(|l| l.is_book));
    }

    #[test]
    fn screen_shows_no_marker_and_no_telling_words() {
        let h = Harness::sample(80, 24);
        let screen = h.screen().to_lowercase();
        assert!(screen.contains("lorem"));
        for w in ["book", "reader", "okuyucu"] {
            assert!(!screen.contains(w), "{w}");
        }
    }

    #[test]
    fn page_back_returns_to_the_previous_screens() {
        let mut h = Harness::sample(80, 24);
        let first = h.visible();
        h.press(&["space"]);
        let second = h.visible();
        h.press(&["space", "pageup"]);
        assert_eq!(h.visible(), second);
        h.press(&["u"]);
        assert_eq!(h.visible(), first);
        h.press(&["u"]);
        assert_eq!(h.visible(), first);
        h.press(&["space"]);
        let after_space = h.visible();
        h.press(&["u", "pagedown"]);
        assert_eq!(h.visible(), after_space);
    }

    #[test]
    fn autosave_writes_progress_every_ten_seconds() {
        let mut h = Harness::sample(80, 24);
        h.press(&["space"]);
        assert!(load_progress(&h.state_dir()).is_empty());
        h.advance(Duration::from_secs(11));
        assert_eq!(load_progress(&h.state_dir()).len(), 1);
    }

    // -- test_no_filename_on_screen.py --

    #[test]
    fn bad_input_at_startup_hides_the_file_name() {
        let d = tempfile::tempdir().unwrap();
        let odd = d.path().join("SucVeCeza");
        std::fs::write(&odd, b"\x00\x01 just some opaque bytes \x02").unwrap();
        for path in [d.path().join("Suç ve Ceza.epub"), odd] {
            let app = App::new(
                Some(path),
                cfg(),
                d.path().join("st"),
                80,
                24,
                Instant::now(),
            );
            let text: String = app
                .feed
                .history_lines()
                .iter()
                .map(Line::plain)
                .chain([app.status_text()])
                .collect::<Vec<_>>()
                .join("\n")
                .to_lowercase();
            assert!(text.contains("error:"), "{text}");
            for part in ["suç", "suc", "ceza", "epub"] {
                assert!(!text.contains(part), "{part} in {text}");
            }
        }
    }
}
