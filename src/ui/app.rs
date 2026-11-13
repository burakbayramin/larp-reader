//! The interactive reader (`ui/app.py`), as a state machine the terminal loop drives.
//!
//! Keys, timer deadlines and resizes come in through `handle_key`, `on_time`
//! and `resize`; `draw` paints the current state. Time is always passed in, so
//! tests can drive the app without a terminal and without waiting.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;

use rand::SeedableRng;

use super::feed::Feed;
use super::input::{InputEvent, LineInput};
use super::menu::{Menu, MenuResult, section_label};
use super::shell::{shell_output, shell_prompt};
use super::style::{base, style};
use super::theme::{self, THEMES};
use crate::book::{Book, Pos, match_spans};
use crate::errors::FccError;
use crate::loaders::{load_book, supported_extensions};
use crate::masks::base::{error_lines, plain_line};
use crate::masks::time::civil;
use crate::masks::{LANGS, Line, MASK_NAMES, Mask, Rng, get_mask, seeded};
use crate::noise::MAX_NOISE;
use crate::pace::{learn, line_delay, step_wpm, words_in};
use crate::session::Session;
use crate::state::{Config, Entry, last_entry, load_progress, save_config, save_progress};

pub const MIN_WIDTH: usize = 40;
pub const AUTOSAVE: Duration = Duration::from_secs(10);
pub const FLASH: Duration = Duration::from_secs(2);
/// Filler lines per second while hidden.
pub const PANIC_RATE: f64 = 4.5;
const WARNING: &str = "warning: window too narrow";
const STATUS_FG: &str = "#858585";
const FG: &str = "#d4d4d4";
const SAVED_MSG: &str = "cache saved";
const NO_MARKS_MSG: &str = "cache empty";
const NOT_FOUND_MSG: &str = "Pattern not found";
const HIGHLIGHT: &str = "reverse";
/// One per digit key.
const LISTED_INPUTS: usize = 9;
const PANEL_WIDTH: u16 = 38;
const KEYS: &[(&str, &str)] = &[
    ("space", "page down"),
    ("u, pgup", "page up"),
    ("j / k", "line down / up"),
    ("] / [", "next / prev section"),
    ("/", "find"),
    ("f / F", "find next / prev"),
    ("a", "auto scroll"),
    ("+ / -", "reading rate"),
    ("m / M", "next / prev view"),
    ("l / L", "next / prev language"),
    ("n / N", "more / less filler"),
    ("o / O", "next / prev input"),
    ("1-9", "open listed input"),
    ("b", "mark this place"),
    ("' / \"", "next / prev mark"),
    (":", "config"),
    ("`", "hide / unhide"),
    ("esc", "this panel"),
    ("q", "quit"),
];

/// The flash after a search wrapped around the end (forward) or the start (backward).
fn wrap_msg(backward: bool) -> &'static str {
    if backward {
        "search hit TOP, continuing at BOTTOM"
    } else {
        "search hit BOTTOM, continuing at TOP"
    }
}

/// The end of `line` that fits in `cols` cells: an input bar scrolls sideways
/// like a terminal, so the typed text and the cursor stay in view.
fn tail_fitting(line: &str, cols: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut kept = vec![];
    let mut used = 0;
    for c in line.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > cols {
            break;
        }
        used += w;
        kept.push(c);
    }
    kept.iter().rev().collect()
}

/// `line` with every match of `query` inside its text segments shown reversed (like grep --color).
pub fn highlight(line: &Line, query: &str) -> Line {
    let mut segments = vec![];
    for (text, style) in &line.segments {
        let spans = if style.split_whitespace().any(|t| t == "book") {
            match_spans(text, query)
        } else {
            vec![]
        };
        if spans.is_empty() {
            segments.push((text.clone(), style.clone()));
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
        let mut at = 0;
        for (start, end) in spans {
            if start > at {
                segments.push((piece(at, start), style.clone()));
            }
            segments.push((piece(start, end), format!("{style} {HIGHLIGHT}")));
            at = end;
        }
        if at < chars.len() {
            segments.push((piece(at, chars.len()), style.clone()));
        }
    }
    Line {
        segments,
        is_book: line.is_book,
        pos: line.pos,
    }
}

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
    /// The time of the event being handled.
    now: Instant,
    /// Auto mode: lines of the paragraph being streamed, not yet shown.
    queue: VecDeque<Line>,
    /// Words of the newest text on screen, still to be read.
    last_words: usize,
    /// When the stream shows its next line.
    next_tick: Option<Instant>,
    /// Reading pace: which manual screen is up, since when, and the text lines seen on it.
    view_key: Option<(String, Pos, usize)>,
    view_since: Instant,
    view_seen: HashMap<(Option<Pos>, String), usize>,
    pub view_words: usize,
    /// The next redraw only scrolled by a line: keep timing the screen.
    line_step: bool,
    /// Filler for the hidden screen: unseeded, it never needs to repeat.
    panic_rng: Rng,
    prompt: String,
    /// The fake shell's input line, while it is up.
    pub shell: Option<LineInput>,
    /// The search input line, while it is up.
    pub search: Option<LineInput>,
    /// The last search; its matches are highlighted.
    pub query: String,
    /// The settings screen, while it is up.
    pub menu: Option<Menu>,
    /// The side panel with the keys and the folder's files.
    pub panel: bool,
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
        theme::set(&config.theme, theme::current().1);
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
            now,
            queue: VecDeque::new(),
            last_words: 0,
            next_tick: None,
            view_key: None,
            view_since: now,
            view_seen: HashMap::new(),
            view_words: 0,
            line_step: false,
            panic_rng: Rng::seed_from_u64(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(1),
            ),
            prompt: shell_prompt(),
            shell: None,
            search: None,
            query: String::new(),
            menu: None,
            panel: false,
        };
        if let Some(e) = mask_error {
            app.notice.extend(error_lines(&e, app.width()));
        }
        if let Err(e) = app.open() {
            app.notice.extend(error_lines(&e, app.width()));
        }
        if app.config.auto && app.session.is_some() {
            app.auto = true;
        }
        app.redraw();
        app.restart_timer();
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
        self.queue.clear();
        let width = self.width();
        let mut lines = vec![];
        if width < MIN_WIDTH {
            lines.push(plain_line(
                WARNING.chars().take(width).collect::<String>(),
                "#d29922",
            ));
        }
        lines.extend(self.notice.iter().cloned());
        let view = self.view_height();
        let mut watched = None;
        if let Some(s) = self.session.as_mut() {
            let at_top = s.book.index_of(s.pos()) == 0 && (self.auto || s.line_offset == 0);
            if at_top {
                let id = s.book.id.clone();
                lines.extend(s.mask_mut().header(&mut seeded(&id), width));
            }
            if self.auto {
                let para = s.paragraph_lines(s.pos());
                self.last_words = words_in(&para);
                lines.extend(para);
            } else {
                let rows = view.saturating_sub(lines.len());
                let text = s.window(rows + 1);
                self.more_below = text.len() > rows;
                self.page_rows = rows.min(text.len());
                let shown: Vec<Line> = text.into_iter().take(rows).collect();
                watched = Some(((s.book.id.clone(), s.pos(), s.line_offset), shown.clone()));
                lines.extend(shown);
            }
        }
        if let Some((key, shown)) = watched {
            self.watch_view(key, &shown);
        }
        self.feed.set_lines(self.highlighted(lines));
    }

    /// Time a manual screen from when it first shows.
    ///
    /// Scrolling by a line keeps the clock running and adds the lines it brings
    /// in, so a page turn afterwards still divides everything read by the time
    /// it really took.
    fn watch_view(&mut self, key: (String, Pos, usize), shown: &[Line]) {
        let step = std::mem::take(&mut self.line_step);
        if self.view_key.as_ref() == Some(&key) {
            return;
        }
        let seen: HashMap<(Option<Pos>, String), usize> = shown
            .iter()
            .filter(|l| l.is_book)
            .map(|l| ((l.pos, l.plain()), words_in(std::slice::from_ref(l))))
            .collect();
        if step && self.view_key.is_some() {
            self.view_seen.extend(seen);
        } else {
            self.view_since = self.now;
            self.view_seen = seen;
        }
        self.view_key = Some(key);
        self.view_words = self.view_seen.values().sum();
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
        self.draw_with(frame, base());
    }

    /// Draw on a screen painted with `base` (the theme's background and text).
    pub fn draw_with(&self, frame: &mut Frame, base: Style) {
        if let Some(menu) = &self.menu {
            menu.draw_with(frame, base);
            return;
        }
        frame.render_widget(ratatui::widgets::Block::default().style(base), frame.area());
        let [feed, status] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());
        frame.render_widget(&self.feed, feed);
        let bar = match (&self.shell, &self.search) {
            (Some(f), _) => Some(format!("{}{}", self.prompt, f.value)),
            (None, Some(f)) => Some(format!("/{}", f.value)),
            _ => None,
        };
        if let Some(line) = bar {
            let line = tail_fitting(&line, status.width.saturating_sub(1) as usize);
            let x = status.x + unicode_width::UnicodeWidthStr::width(line.as_str()) as u16;
            frame.render_widget(Paragraph::new(line).style(base.patch(style(FG))), status);
            frame.set_cursor_position((x, status.y));
        } else {
            frame.render_widget(
                Paragraph::new(self.status_text()).style(base.patch(style(STATUS_FG))),
                status,
            );
        }
        if self.panel {
            self.draw_panel(frame);
        }
    }

    fn draw_panel(&self, frame: &mut Frame) {
        use ratatui::widgets::{Block, Borders, Clear, Padding};
        let area = frame.area();
        let width = PANEL_WIDTH.min(area.width);
        let rect = ratatui::layout::Rect {
            x: area.x + area.width - width,
            y: area.y,
            width,
            height: area.height,
        };
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(style("#3c3c3c"))
            .padding(Padding::new(2, 2, 1, 1))
            .style(style("#cccccc on #252526"));
        frame.render_widget(Clear, rect);
        frame.render_widget(Paragraph::new(self.panel_text()).block(block), rect);
    }

    /// The side panel's text: the keys, then the folder's openable files.
    pub fn panel_text(&self) -> Vec<ratatui::text::Line<'static>> {
        use ratatui::text::{Line as TLine, Span};
        let (bold, key_style, text_style, dim) = (
            style("bold #d4d4d4"),
            style("#dcdcaa"),
            style("#cccccc"),
            style("#858585"),
        );
        let mut out = vec![TLine::styled("keys", bold)];
        for (k, what) in KEYS {
            out.push(TLine::from(vec![
                Span::styled(format!("{k:<9}"), key_style),
                Span::styled(*what, text_style),
            ]));
        }
        let files = self.inputs();
        if !files.is_empty() {
            out.push(TLine::raw(""));
            out.push(TLine::styled("inputs", bold));
            let current = self.entry_path.as_deref().map(PathBuf::from);
            for (n, f) in files.iter().take(LISTED_INPUTS).enumerate() {
                let here = Some(f) == current.as_ref();
                let name = f
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let mark = if here {
                    ">".to_string()
                } else {
                    (n + 1).to_string()
                };
                out.push(TLine::from(vec![
                    Span::styled(format!("{mark} "), key_style),
                    Span::styled(name, if here { bold } else { text_style }),
                ]));
            }
            if files.len() > LISTED_INPUTS {
                out.push(TLine::styled(
                    format!("  +{} more (o / O)", files.len() - LISTED_INPUTS),
                    dim,
                ));
            }
        }
        out
    }

    /// The openable files next to the current one, in name order.
    pub fn inputs(&self) -> Vec<PathBuf> {
        let Some(current) = self.entry_path.as_deref().map(PathBuf::from) else {
            return vec![];
        };
        let known = supported_extensions();
        let mut found: Vec<PathBuf> = current
            .parent()
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_file() && known.contains(&crate::loaders::suffix(p).as_str()))
                    .collect()
            })
            .unwrap_or_default();
        if !found.contains(&current) {
            found.push(current);
        }
        found.sort_by_key(|f| {
            f.file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        });
        found
    }

    /// The mask in use: the open session's, or the one kept for errors.
    fn mask_mut(&mut self) -> &mut dyn Mask {
        match self.session.as_mut() {
            Some(s) => s.mask_mut(),
            None => self
                .spare_mask
                .get_or_insert_with(|| resolve_mask(&self.config).0)
                .as_mut(),
        }
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
        self.now = now;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.code == KeyCode::Char('`') {
            self.toggle_panic(); // works from anywhere, bars and menus included
            return;
        }
        if ctrl && key.code == KeyCode::Char('q') {
            self.quit_and_save();
            return;
        }
        if let Some(field) = self.shell.as_mut() {
            if let InputEvent::Submit(command) = field.handle(key) {
                self.run_shell(&command);
            }
            return; // a shell takes every key: nothing here moves the text
        }
        if let Some(menu) = self.menu.as_mut() {
            let result = menu.handle(key);
            if result != MenuResult::Stay {
                self.menu = None;
                self.on_menu_result(result);
            }
            return;
        }
        if let Some(field) = self.search.as_mut() {
            match field.handle(key) {
                InputEvent::Submit(query) => {
                    self.search = None;
                    self.query = query.trim().to_string();
                    match self.session.as_ref().map(Session::pos) {
                        Some(pos) if !self.query.is_empty() => self.find(pos, false, false),
                        _ => self.redraw(),
                    }
                }
                InputEvent::Ignored if key.code == KeyCode::Esc => self.search = None,
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('q') if ctrl => self.quit_and_save(),
            KeyCode::Char('q') => self.quit_and_save(),
            KeyCode::Char(' ') | KeyCode::PageDown => self.page(),
            KeyCode::Char('u') | KeyCode::PageUp => self.page_back(),
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::Char('a') => self.toggle_auto(),
            KeyCode::Char('+') => self.set_wpm(step_wpm(self.config.wpm, 1)),
            KeyCode::Char('-') => self.set_wpm(step_wpm(self.config.wpm, -1)),
            KeyCode::Char(']') => self.chapter(1),
            KeyCode::Char('[') => self.chapter(-1),
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('f') => self.find_next(false),
            KeyCode::Char('F') => self.find_next(true),
            KeyCode::Char('b') => self.bookmark(),
            KeyCode::Char('m') => self.cycle_mask(1),
            KeyCode::Char('M') => self.cycle_mask(-1),
            KeyCode::Char('l') => self.cycle_lang(1),
            KeyCode::Char('L') => self.cycle_lang(-1),
            KeyCode::Char('n') => self.change_noise(1),
            KeyCode::Char('N') => self.change_noise(-1),
            KeyCode::Char('o') => self.cycle_input(1),
            KeyCode::Char('O') => self.cycle_input(-1),
            KeyCode::Char(c @ '1'..='9') => self.open_listed(c as usize - '0' as usize),
            KeyCode::Esc if !self.panic => self.panel = !self.panel,
            KeyCode::Char(':') => self.open_menu(),
            KeyCode::Char('t') => self.cycle_theme(),
            KeyCode::Char('\'') => self.jump_mark(true),
            KeyCode::Char('"') => self.jump_mark(false),
            _ => {}
        }
    }

    fn step(&mut self, n: isize) {
        self.line_step = true;
        self.scroll(n);
        self.line_step = false; // in case nothing was redrawn
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
        if self.session.is_some() && !self.auto && !self.panic && self.more_below {
            // paging on means the screen was read: let it tune the reading rate
            let read = self
                .now
                .saturating_duration_since(self.view_since)
                .as_secs_f64();
            if let Some(rate) = learn(self.config.wpm, self.view_words, read)
                && rate != self.config.wpm
            {
                self.config.wpm = rate;
                self.persist_config();
            }
        }
        self.scroll(self.page_rows as isize);
    }

    pub fn toggle_panic(&mut self) {
        self.panic = !self.panic;
        self.stop_timer();
        self.feed.clear();
        self.queue.clear();
        self.view_key = None; // time spent hidden is not reading time
        self.search = None;
        self.menu = None;
        self.panel = false;
        let shell = self.config.hide == "shell";
        if self.panic {
            self.feed.set_bottom_anchored(true);
            let (width, rows) = (self.width(), self.view_height() - usize::from(shell));
            let mut rng = self.panic_rng.clone();
            let lines: Vec<Line> = (0..rows)
                .map(|_| self.mask_mut().noise(&mut rng, width))
                .collect();
            self.panic_rng = rng;
            self.feed.append(lines);
            if shell {
                self.shell = Some(LineInput::default());
            }
        } else {
            self.shell = None;
            self.redraw();
        }
        self.restart_timer();
    }

    /// Echo a command at the prompt and print something believable for it.
    fn run_shell(&mut self, command: &str) {
        let command = command.trim().to_string();
        let width = self.width();
        let mut rng = self.panic_rng.clone();
        let out = shell_output(&command, self.mask_mut(), &mut rng, width);
        self.panic_rng = rng;
        match out {
            None => self.feed.clear(),
            Some(out) => {
                let mut lines = vec![plain_line(format!("{}{command}", self.prompt), FG)];
                lines.extend(out);
                self.feed.append(lines);
            }
        }
    }

    /// Mark the last search's matches inside the text.
    fn highlighted(&self, lines: Vec<Line>) -> Vec<Line> {
        if self.query.is_empty() {
            return lines;
        }
        lines
            .into_iter()
            .map(|l| {
                if l.is_book {
                    highlight(&l, &self.query)
                } else {
                    l
                }
            })
            .collect()
    }

    /// Show the text from the top of paragraph `pos`.
    fn go(&mut self, pos: Pos) {
        if let Some(s) = self.session.as_mut() {
            s.set_pos(pos);
        }
        self.notice.clear();
        self.redraw();
    }

    fn chapter(&mut self, step: i32) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        if self.panic {
            return;
        }
        let pos = s.pos();
        let target = if step > 0 {
            s.book.next_chapter(pos)
        } else if pos.1 == 0 && s.line_offset > 0 {
            Some(pos) // back to the top of a section that starts here
        } else {
            s.book.prev_chapter(pos)
        };
        if let Some(t) = target {
            self.go(t);
        }
    }

    fn open_search(&mut self) {
        if self.session.is_some() && !self.panic {
            self.panel = false;
            self.search = Some(LineInput::default());
        }
    }

    // -- views, filler, files and the menu ---------------------------------------

    /// Switch the disguise in place, staying on the current paragraph.
    fn use_mask(&mut self, name: &str, lang: &str) {
        let Ok(mask) = get_mask(name, lang) else {
            return;
        };
        if let Some(s) = self.session.as_mut() {
            s.set_mask(mask);
            let pos = s.pos();
            s.set_pos(pos); // new layout: restart at the paragraph's top
        } else {
            self.spare_mask = Some(mask);
        }
        self.config.mask = name.to_string();
        self.config.lang = lang.to_string();
        self.persist_config();
        self.redraw();
    }

    fn current_mask_name(&self) -> String {
        match &self.session {
            Some(s) => s.mask().name().to_string(),
            None => self
                .spare_mask
                .as_ref()
                .map(|m| m.name().to_string())
                .unwrap_or_else(|| self.config.mask.clone()),
        }
    }

    fn cycle_mask(&mut self, step: isize) {
        let name = self.current_mask_name();
        let i = MASK_NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| i as isize)
            .unwrap_or(-1);
        let next = MASK_NAMES[(i + step).rem_euclid(MASK_NAMES.len() as isize) as usize];
        let lang = self.config.lang.clone();
        self.use_mask(next, &lang);
    }

    /// Next source language; only the code mask has one, so this also returns to it.
    fn cycle_lang(&mut self, step: isize) {
        let i = LANGS
            .iter()
            .position(|l| *l == self.config.lang)
            .map(|i| i as isize)
            .unwrap_or(-1);
        let next = LANGS[(i + step).rem_euclid(LANGS.len() as isize) as usize];
        self.use_mask("code", next);
    }

    fn use_theme(&mut self, name: &str) {
        self.config.theme = name.to_string();
        theme::set(name, theme::current().1);
    }

    fn cycle_theme(&mut self) {
        if self.panic {
            return;
        }
        let i = THEMES
            .iter()
            .position(|t| *t == self.config.theme)
            .unwrap_or(0);
        let next = THEMES[(i + 1) % THEMES.len()];
        self.use_theme(next);
        self.persist_config();
    }

    fn change_noise(&mut self, delta: i32) {
        let level = (self.config.noise as i32 + delta).clamp(0, MAX_NOISE as i32) as u8;
        self.config.noise = level;
        if let Some(s) = self.session.as_mut() {
            s.set_noise(level);
        }
        self.persist_config();
        self.redraw();
    }

    fn cycle_input(&mut self, step: isize) {
        let files = self.inputs();
        if self.panic || files.len() < 2 {
            return;
        }
        let current = self.entry_path.as_deref().map(PathBuf::from);
        let i = files
            .iter()
            .position(|f| Some(f) == current.as_ref())
            .unwrap_or(0) as isize;
        let next = files[(i + step).rem_euclid(files.len() as isize) as usize].clone();
        self.open_from_menu(&next);
    }

    fn open_listed(&mut self, n: usize) {
        let files = self.inputs();
        if self.panic || !self.panel || n > files.len().min(LISTED_INPUTS) {
            return;
        }
        let target = files[n - 1].clone();
        if Some(target.to_string_lossy().as_ref()) != self.entry_path.as_deref() {
            self.open_from_menu(&target);
        }
    }

    /// Open another file, keeping this one's place saved first.
    fn open_from_menu(&mut self, path: &Path) {
        self.try_save();
        match self.load(path) {
            Err(e) => self.notice = error_lines(&e, self.width()),
            Ok(()) => {
                self.notice.clear();
                self.queue.clear();
            }
        }
        self.redraw();
        self.restart_timer();
    }

    fn open_menu(&mut self) {
        if self.panic {
            return;
        }
        let (marks, sections) = match &self.session {
            Some(s) => (
                self.bookmarks
                    .iter()
                    .map(|m| (m[0], m[1]))
                    .filter(|p| valid_pos(&s.book, *p) == *p)
                    .map(|p| (format!("L{}", s.book.index_of(p) + 1), p))
                    .collect(),
                s.book
                    .chapter_list()
                    .into_iter()
                    .enumerate()
                    .map(|(n, (ci, title))| (section_label(n + 1, title), (ci, 0)))
                    .collect(),
            ),
            None => (vec![], vec![]),
        };
        let mask = self.current_mask_name();
        self.menu = Some(Menu::new(&mask, &self.config, marks, sections));
    }

    fn on_menu_result(&mut self, result: MenuResult) {
        match result {
            MenuResult::Stay | MenuResult::Close => {}
            MenuResult::Panic => {
                if !self.panic {
                    self.toggle_panic();
                }
            }
            MenuResult::Apply {
                mask,
                lang,
                wpm,
                noise,
                hide,
                theme,
            } => {
                self.config.noise = noise;
                self.config.wpm = wpm;
                self.config.hide = hide;
                self.use_theme(&theme);
                if let Some(s) = self.session.as_mut() {
                    s.set_noise(noise);
                }
                self.use_mask(&mask, &lang); // saves the config and redraws
                self.restart_timer();
            }
            MenuResult::Open(p) => {
                let path = PathBuf::from(p);
                let path = match (path.strip_prefix("~"), std::env::var_os("HOME")) {
                    (Ok(rest), Some(home)) => PathBuf::from(home).join(rest),
                    _ => path,
                };
                self.open_from_menu(&path);
            }
            MenuResult::Jump(pos) => {
                if let Some(s) = self.session.as_ref() {
                    let pos = valid_pos(&s.book, pos);
                    self.go(pos);
                }
            }
        }
    }

    fn find(&mut self, start: Pos, backward: bool, wrapped: bool) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        match s.book.find(&self.query, start, backward) {
            None => {
                self.redraw();
                self.flash(NOT_FOUND_MSG, self.now);
            }
            Some((pos, wrapped_now)) => {
                self.go(pos);
                if wrapped || wrapped_now {
                    self.flash(wrap_msg(backward), self.now);
                }
            }
        }
    }

    fn find_next(&mut self, backward: bool) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        if self.panic || self.query.is_empty() {
            return;
        }
        let book = &s.book;
        let start = if backward {
            book.prev_pos(s.pos())
        } else {
            book.next_pos(s.pos())
        };
        let wrapped = start.is_none();
        let start = start.or_else(|| {
            let chapters = book.chapter_list();
            if backward {
                let (last, _) = *chapters.last()?;
                Some((last, book.chapters[last].paragraphs.len() - 1))
            } else {
                chapters.first().map(|(c, _)| (*c, 0))
            }
        });
        if let Some(start) = start {
            self.find(start, backward, wrapped);
        }
    }

    fn bookmark(&mut self) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        if self.panic {
            return;
        }
        let mark = [s.pos().0, s.pos().1];
        if !self.bookmarks.contains(&mark) {
            self.bookmarks.push(mark);
            self.try_save();
        }
        self.flash(SAVED_MSG, self.now);
    }

    fn jump_mark(&mut self, forward: bool) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        if self.panic {
            return;
        }
        let mut marks: Vec<(usize, Pos)> = self
            .bookmarks
            .iter()
            .map(|m| (m[0], m[1]))
            .filter(|p| valid_pos(&s.book, *p) == *p)
            .map(|p| (s.book.index_of(p), p))
            .collect();
        if marks.is_empty() {
            self.flash(NO_MARKS_MSG, self.now);
            return;
        }
        marks.sort();
        let here = s.book.index_of(s.pos());
        let target = if forward {
            marks.iter().find(|(i, _)| *i > here).unwrap_or(&marks[0]).1
        } else {
            marks
                .iter()
                .rev()
                .find(|(i, _)| *i < here)
                .unwrap_or(&marks[marks.len() - 1])
                .1
        };
        self.go(target);
    }

    fn toggle_auto(&mut self) {
        if self.session.is_none() || self.panic {
            return;
        }
        self.auto = !self.auto;
        self.notice.clear();
        self.redraw();
        self.restart_timer();
    }

    fn set_wpm(&mut self, wpm: u32) {
        self.config.wpm = wpm;
        self.persist_config();
        if !self.panic {
            self.flash(&format!("throttle {wpm}/min"), self.now);
        }
        self.restart_timer();
    }

    fn persist_config(&self) {
        let _ = save_config(&self.state_dir, &self.config); // best effort: the choice still applies now
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
        if let Some(t) = self.next_tick {
            at = at.min(t);
        }
        if let Some((_, until)) = &self.flash {
            at = at.min(*until);
        }
        at
    }

    pub fn on_time(&mut self, now: Instant) {
        self.now = now;
        if self.next_tick.is_some_and(|t| now >= t) {
            self.timer_fired();
        }
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

    fn stop_timer(&mut self) {
        self.next_tick = None;
    }

    fn restart_timer(&mut self) {
        self.stop_timer();
        if self.panic {
            if self.config.hide != "shell" {
                self.next_tick = Some(self.now + Duration::from_secs_f64(1.0 / PANIC_RATE));
            } // a shell waits for input
            return;
        }
        if self.auto && self.session.is_some() {
            // the text already on screen gets its reading time before the next line
            self.next_tick = Some(
                self.now + Duration::from_secs_f64(line_delay(self.last_words, self.config.wpm)),
            );
        }
    }

    fn timer_fired(&mut self) {
        self.next_tick = None;
        self.tick();
        if self.panic {
            self.restart_timer();
            return;
        }
        if self.auto && !self.panic && self.next_tick.is_none() {
            self.next_tick = Some(
                self.now + Duration::from_secs_f64(line_delay(self.last_words, self.config.wpm)),
            );
        }
    }

    /// One stream step: the next line of text, with the filler before it.
    pub fn tick(&mut self) {
        if self.panic {
            let width = self.width();
            let mut rng = self.panic_rng.clone();
            let line = self.mask_mut().noise(&mut rng, width);
            self.panic_rng = rng;
            self.feed.append(vec![line]);
            return;
        }
        if !self.auto {
            return;
        }
        let Some(s) = self.session.as_mut() else {
            return;
        };
        let mut out = vec![];
        let mut finished = false;
        loop {
            if self.queue.is_empty() {
                if !s.advance(1) {
                    finished = true;
                    break;
                }
                self.queue = s.paragraph_lines(s.pos()).into();
            }
            let line = self.queue.pop_front().expect("refilled above");
            let is_book = line.is_book;
            out.push(line);
            if is_book {
                break;
            }
        }
        self.last_words = words_in(&out);
        if !out.is_empty() {
            let out = self.highlighted(out);
            self.feed.append(out);
        }
        if finished {
            // end of text: back to manual mode so the paging keys work again
            self.stop_timer();
            self.auto = false;
            self.redraw();
        }
    }

    pub fn timer_due(&self) -> Option<Instant> {
        self.next_tick
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

        /// The screen's cells drawn with the base style of `theme`.
        pub fn cells(&self, theme: &str) -> ratatui::buffer::Buffer {
            let base = crate::ui::style::base_in(theme, theme::Depth::TrueColor);
            let mut t = Terminal::new(TestBackend::new(self.app.cols as u16, self.app.rows as u16))
                .unwrap();
            t.draw(|f| self.app.draw_with(f, base)).unwrap();
            t.backend().buffer().clone()
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
    fn every_cell_gets_the_themes_background() {
        use ratatui::style::Color;
        let h = Harness::sample(80, 24);
        for (name, bg) in [
            ("light", Color::Rgb(0xff, 0xff, 0xff)),
            ("dark", Color::Rgb(0x1e, 0x1e, 0x1e)),
            ("dark-ansi", Color::Reset),
            ("light-ansi", Color::Reset),
        ] {
            let buf = h.cells(name);
            for y in 0..buf.area.height {
                for x in 0..buf.area.width {
                    assert_eq!(buf[(x, y)].bg, bg, "{name} at {x},{y}");
                }
            }
        }
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

    // -- test_app_auto_panic.py (auto) and test_app_hardening.py --

    fn sample_with(n: usize, config: Config, cols: u16, rows: u16) -> Harness {
        let d = tempfile::tempdir().unwrap();
        let p = make_txt(d.path(), n);
        Harness::new(Some(p), config, d, cols, rows)
    }

    fn noisy() -> Config {
        Config { noise: 3, ..cfg() }
    }

    fn book_count(h: &Harness) -> usize {
        h.app
            .feed
            .history_lines()
            .iter()
            .filter(|l| l.is_book)
            .count()
    }

    #[test]
    fn auto_tick_appends_one_book_line_plus_filler() {
        let mut h = sample_with(10, noisy(), 80, 24);
        h.press(&["a"]);
        assert!(h.app.auto);
        for _ in 0..6 {
            let (books, all) = (book_count(&h), h.app.feed.history_lines().len());
            h.app.tick();
            assert_eq!(book_count(&h), books + 1);
            assert!(h.app.feed.history_lines().len() > all);
        }
    }

    #[test]
    fn auto_is_bottom_anchored_and_capped() {
        let mut h = sample_with(400, noisy(), 80, 24);
        h.press(&["a"]);
        for _ in 0..40 {
            h.app.tick();
        }
        let hist = h.app.feed.history_lines().to_vec();
        assert_eq!(h.visible(), hist[hist.len() - h.visible().len()..].to_vec());
        for _ in 0..1500 {
            h.app.tick();
        }
        assert_eq!(h.app.feed.history_lines().len(), crate::ui::feed::MAX_LINES);
    }

    #[test]
    fn wpm_is_clamped_stepped_shown_and_saved() {
        let mut h = sample_with(10, Config { wpm: 200, ..cfg() }, 80, 24);
        h.press(&["+"]);
        assert_eq!(h.app.config.wpm, 220);
        assert!(h.status().contains("220/min"));
        h.press(&["-"]);
        assert_eq!(h.app.config.wpm, 200);
        assert_eq!(crate::state::load_config(&h.state_dir()).wpm, 200);
        h.press(&["+"; 60]);
        assert_eq!(h.app.config.wpm, 1500);
        h.press(&["-"; 80]);
        assert_eq!(h.app.config.wpm, 60);
        h.advance(FLASH + Duration::from_millis(1));
        assert!(!h.status().contains("/min"));
    }

    #[test]
    fn config_auto_and_wpm_apply_at_startup() {
        let h = sample_with(
            10,
            Config {
                auto: true,
                wpm: 400,
                ..cfg()
            },
            80,
            24,
        );
        assert!(h.app.auto);
        assert_eq!(h.app.config.wpm, 400);
        assert!(h.app.timer_due().is_some());
    }

    #[test]
    fn auto_reaches_the_end_stops_and_manual_keys_work_again() {
        let mut h = sample_with(3, cfg(), 80, 24);
        h.press(&["a"]);
        for _ in 0..200 {
            h.app.tick();
        }
        assert_eq!(h.session().progress(), (3, 3));
        assert_eq!(h.session().pos(), (0, 2));
        assert!(!h.app.auto && h.app.timer_due().is_none());
        let n = h.app.feed.history_lines().len();
        h.app.tick();
        assert_eq!(h.app.feed.history_lines().len(), n);
        let top = h.visible()[0].clone();
        h.press(&["k"]);
        assert_ne!(h.visible()[0], top);
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

    #[test]
    fn auto_waits_by_the_words_on_screen_then_by_each_line() {
        let mut h = paced(300);
        h.press(&["a"]);
        let first = words_in(&h.session().paragraph_lines((0, 0)));
        let due = h.app.timer_due().unwrap();
        assert_eq!(due, h.now + Duration::from_secs_f64(line_delay(first, 300)));
        let before = h.app.feed.history_lines().len();
        h.now = due;
        h.app.on_time(due);
        let new = h.app.feed.history_lines()[before..].to_vec();
        assert!(words_in(&new) > 0);
        assert_eq!(
            h.app.timer_due().unwrap(),
            due + Duration::from_secs_f64(line_delay(words_in(&new), 300))
        );
    }

    #[test]
    fn paging_learns_the_reading_rate() {
        let mut h = paced(200);
        let words = h.app.view_words;
        assert!(words > 20);
        h.now += Duration::from_secs(20);
        h.press(&["space"]);
        let want = learn(200, words, 20.0).unwrap();
        assert!(want != 200 && h.app.config.wpm == want);
        assert_eq!(crate::state::load_config(&h.state_dir()).wpm, want);
    }

    #[test]
    fn quick_paging_and_line_scrolling_do_not_learn() {
        let mut h = paced(200);
        h.now += Duration::from_millis(500);
        h.press(&["space"]); // skimmed
        h.now += Duration::from_secs(30);
        h.press(&["j", "j"]);
        assert_eq!(h.app.config.wpm, 200);
    }

    #[test]
    fn line_scrolling_keeps_the_clock_and_counts_the_new_lines() {
        // a screen read for 20 s, a line scrolled, then paged 3 s later: 23 s of reading
        let mut h = paced(200);
        let before = h.app.view_words;
        h.now += Duration::from_secs(20);
        h.press(&["j", "j"]);
        h.now += Duration::from_secs(3);
        let read = h.app.view_words;
        assert!(read >= before);
        h.press(&["space"]);
        assert_eq!(h.app.config.wpm, learn(200, read, 23.0).unwrap());
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

    #[test]
    fn panic_removes_every_word_of_the_text() {
        let mut h = Harness::sample(80, 24);
        let words = text_words(&mut h);
        assert!(!leaked(&h, &words).is_empty()); // the text WAS visible
        h.press(&["a"]);
        for _ in 0..5 {
            h.app.tick();
        }
        h.press(&["`"]);
        assert!(h.app.panic);
        assert!(h.app.feed.history_lines().len() >= 23); // a screenful before the first tick
        for _ in 0..30 {
            h.app.tick();
        }
        assert!(!shown(&h).trim().is_empty());
        assert_eq!(leaked(&h, &words), Vec::<String>::new());
        assert!(!h.app.feed.history_lines().iter().any(|l| l.is_book));
    }

    #[test]
    fn panic_toggle_returns_to_the_same_place_and_mode() {
        let mut h = Harness::sample(80, 24);
        h.press(&["down", "down"]);
        let pos = h.session().pos();
        h.press(&["`"]);
        for _ in 0..5 {
            h.app.tick();
        }
        h.press(&["space", "down", "a"]); // manual keys do nothing while hidden
        assert_eq!(h.session().pos(), pos);
        assert!(!h.app.auto);
        h.press(&["`"]);
        assert!(!h.app.panic && !h.app.auto);
        assert!(h.visible().iter().any(|l| l.pos == Some(pos)));
        h.press(&["a", "`", "`"]);
        assert!(h.app.auto && !h.app.panic);
        assert_eq!(h.session().pos(), pos);
    }

    #[test]
    fn hidden_stream_flows_on_a_timer() {
        let mut h = Harness::sample(80, 24);
        h.press(&["`"]);
        let n = h.app.feed.history_lines().len();
        let due = h.app.timer_due().unwrap();
        h.now = due;
        h.app.on_time(due);
        assert_eq!(h.app.feed.history_lines().len(), n + 1);
    }

    #[test]
    fn time_spent_hidden_is_not_reading_time() {
        let mut h = paced(200);
        let words = h.app.view_words;
        h.press(&["`"]);
        h.now += Duration::from_secs(120); // someone walked by
        h.press(&["`"]);
        h.now += Duration::from_secs(15);
        h.press(&["space"]);
        assert_eq!(h.app.config.wpm, learn(200, words, 15.0).unwrap());
    }

    fn shell_harness() -> Harness {
        sample_with(
            10,
            Config {
                hide: "shell".into(),
                ..cfg()
            },
            80,
            24,
        )
    }

    fn type_text(h: &mut Harness, text: &str) {
        for c in text.chars() {
            h.app
                .handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), h.now);
        }
    }

    #[test]
    fn shell_hide_shows_a_prompt_that_takes_commands() {
        let mut h = shell_harness();
        let words = text_words(&mut h);
        h.press(&["`"]);
        assert!(h.app.panic && h.app.shell.is_some());
        assert!(
            h.app
                .timer_due()
                .is_none_or(|t| t > h.now + Duration::from_secs(5))
        ); // a shell waits, it does not stream
        let screen = h.screen();
        assert!(
            screen
                .lines()
                .last()
                .unwrap()
                .trim_end()
                .ends_with(['$', '%', '>']),
            "{screen}"
        );
        type_text(&mut h, "ls");
        h.press(&["enter"]);
        let lines: Vec<String> = h.app.feed.history_lines().iter().map(Line::plain).collect();
        assert!(lines.iter().any(|l| l.ends_with(" ls")));
        assert!(lines.join(" ").contains("src"));
        assert_eq!(
            leaked(&h, &words)
                .into_iter()
                .filter(|w| w != "number")
                .count(),
            0
        );
        type_text(&mut h, "jk q");
        assert_eq!((h.session().pos(), h.session().line_offset), ((0, 0), 0));
        assert!(!h.app.should_quit()); // q is just a letter at the prompt
        h.press(&["`"]);
        assert!(!h.app.panic && h.app.shell.is_none());
    }

    #[test]
    fn a_long_prompt_scrolls_so_the_typed_text_stays_in_view() {
        assert_eq!(
            tail_fitting("runner@a-very-long-host-name:~/work$ ls", 12),
            "e:~/work$ ls"
        );
        assert_eq!(tail_fitting("short$ ", 40), "short$ ");
        let mut h = shell_harness();
        h.app.prompt = format!("runner@{}:~/work/fcc$ ", "x".repeat(100));
        h.press(&["`"]);
        type_text(&mut h, "git status");
        assert!(
            h.screen()
                .lines()
                .last()
                .unwrap()
                .trim_end()
                .ends_with("$ git status")
        );
    }

    #[test]
    fn shell_clear_and_unknown_commands() {
        let mut h = shell_harness();
        h.press(&["`"]);
        type_text(&mut h, "clear");
        h.press(&["enter"]);
        assert!(h.app.feed.history_lines().is_empty());
        type_text(&mut h, "frobnicate");
        h.press(&["enter"]);
        let text = shown(&h);
        assert!(
            text.contains("frobnicate")
                && (text.contains("not found") || text.contains("not recognized"))
        );
    }

    // -- test_navigate.py (sections, search, marks) and test_menu_bookmarks.py (b) --

    fn sections_harness() -> Harness {
        let d = tempfile::tempdir().unwrap();
        let mut parts = vec![];
        for c in 0..3 {
            parts.push(format!("# Kısım {c}"));
            parts.extend(
                (0..4).map(|i| format!("chapter{c} para{i} {}", "lorem ipsum ".repeat(30))),
            );
        }
        let p = d.path().join("parts.md");
        std::fs::write(&p, parts.join("\n\n")).unwrap();
        Harness::new(Some(p), cfg(), d, 80, 24)
    }

    #[test]
    fn brackets_jump_between_sections() {
        let mut h = sections_harness();
        h.press(&["]"]);
        assert_eq!(h.session().pos(), (1, 0));
        h.press(&["]", "]"]);
        assert_eq!(h.session().pos(), (2, 0)); // no section after the last
        h.press(&["["]);
        assert_eq!(h.session().pos(), (1, 0));
        h.press(&["j", "j", "j"]);
        assert_ne!((h.session().pos(), h.session().line_offset), ((1, 0), 0));
        h.press(&["["]);
        assert_eq!((h.session().pos(), h.session().line_offset), ((1, 0), 0));
        h.press(&["["]);
        assert_eq!(h.session().pos(), (0, 0));
    }

    #[test]
    fn slash_searches_and_highlights() {
        let mut h = Harness::sample(80, 24);
        h.press(&["/"]);
        assert!(h.app.search.is_some());
        type_text(&mut h, "number 7");
        assert!(h.screen().lines().last().unwrap().starts_with("/number 7"));
        h.press(&["enter"]);
        assert!(h.app.search.is_none());
        assert_eq!(h.session().pos(), (0, 7));
        let hits: Vec<String> = h
            .visible()
            .iter()
            .flat_map(|l| l.segments.clone())
            .filter(|(_, s)| s.split_whitespace().any(|t| t == "reverse"))
            .map(|(t, _)| t)
            .collect();
        assert!(hits.contains(&"number 7".to_string()), "{hits:?}");
    }

    #[test]
    fn f_and_shift_f_walk_matches_and_wrap() {
        let mut h = Harness::sample(80, 24);
        h.press(&["/"]);
        type_text(&mut h, "LOREM");
        h.press(&["enter"]);
        assert_eq!(h.session().pos(), (0, 0));
        h.press(&["f"]);
        assert_eq!(h.session().pos(), (0, 1));
        h.press(&["F", "F"]);
        assert_eq!(h.session().pos(), (0, 9)); // wrapped to the end
        assert!(h.status().contains("continuing"));
    }

    #[test]
    fn missing_pattern_reports_and_stays() {
        let mut h = Harness::sample(80, 24);
        h.press(&["/"]);
        type_text(&mut h, "zzz");
        h.press(&["enter"]);
        assert_eq!(h.session().pos(), (0, 0));
        assert!(h.status().contains("Pattern not found"));
    }

    #[test]
    fn escape_cancels_search_and_backtick_hides() {
        let mut h = Harness::sample(80, 24);
        h.press(&["/", "x", "escape"]);
        assert!(h.app.search.is_none());
        h.press(&["j"]);
        assert!(h.session().line_offset == 1 || h.session().pos() != (0, 0));
        h.press(&["/", "`"]);
        assert!(h.app.panic && h.app.search.is_none());
    }

    #[test]
    fn quote_keys_jump_between_marks() {
        let mut h = Harness::sample(80, 24);
        h.app.bookmarks = vec![[0, 6], [0, 2]];
        h.press(&["'"]);
        assert_eq!(h.session().pos(), (0, 2));
        h.press(&["'"]);
        assert_eq!(h.session().pos(), (0, 6));
        h.press(&["'"]);
        assert_eq!(h.session().pos(), (0, 2)); // wraps
        h.press(&["\""]);
        assert_eq!(h.session().pos(), (0, 6));
    }

    #[test]
    fn quote_without_marks_says_so() {
        let mut h = Harness::sample(80, 24);
        h.press(&["'"]);
        assert_eq!(h.session().pos(), (0, 0));
        assert!(h.status().contains("cache"));
    }

    #[test]
    fn bookmark_is_added_once_saved_and_flashed() {
        let mut h = Harness::sample(100, 24);
        h.press(&["space", "b", "b"]);
        let pos = h.session().pos();
        assert_eq!(h.app.bookmarks, vec![[pos.0, pos.1]]);
        assert_eq!(h.status(), "cache saved");
        let id = h.session().book.id.clone();
        assert_eq!(
            load_progress(&h.state_dir())[&id].bookmarks,
            vec![[pos.0, pos.1]]
        );
        h.advance(FLASH + Duration::from_millis(1));
        assert_ne!(h.status(), "cache saved");
    }

    #[test]
    fn highlight_splits_only_text_segments() {
        let line = Line::book(
            vec![
                ("  12 ".into(), "#858585".into()),
                ("Alfa beta ALFA".into(), "book #d4d4d4".into()),
            ],
            (0, 0),
        );
        let out = highlight(&line, "alfa");
        let texts: Vec<(&str, bool)> = out
            .segments
            .iter()
            .map(|(t, s)| (t.as_str(), s.ends_with("reverse")))
            .collect();
        assert_eq!(
            texts,
            vec![
                ("  12 ", false),
                ("Alfa", true),
                (" beta ", false),
                ("ALFA", true)
            ]
        );
    }

    // -- test_app_switch.py, test_help_and_inputs.py, test_menu_bookmarks.py --

    #[test]
    fn m_and_shift_m_cycle_views_keeping_the_place() {
        let mut h = Harness::sample(100, 24);
        h.press(&["space"]);
        let pos = h.session().pos();
        let mut seen = vec![h.session().mask().name().to_string()];
        for _ in 0..MASK_NAMES.len() {
            h.press(&["m"]);
            seen.push(h.session().mask().name().to_string());
            assert_eq!(h.session().pos(), pos);
            assert!(h.visible().iter().any(|l| l.pos == Some(pos)));
            assert_eq!(
                crate::state::load_config(&h.state_dir()).mask,
                *seen.last().unwrap()
            );
        }
        assert_eq!(
            seen,
            [
                "code", "log", "diff", "test", "build", "git", "docker", "agent", "code"
            ]
        );
        h.press(&["M"]);
        assert_eq!(h.session().mask().name(), "agent");
    }

    #[test]
    fn l_cycles_languages_and_returns_to_code() {
        let mut h = Harness::sample(100, 24);
        h.press(&["m", "l"]);
        assert_eq!(
            (h.session().mask().name(), h.app.config.lang.as_str()),
            ("code", "typescript")
        );
        h.press(&["L", "L"]);
        assert_eq!(h.app.config.lang, "csharp");
        assert_eq!(crate::state::load_config(&h.state_dir()).lang, "csharp");
    }

    #[test]
    fn n_and_shift_n_change_filler_within_bounds() {
        let mut h = Harness::sample(100, 24);
        h.press(&["n", "n"]);
        assert_eq!((h.app.config.noise, h.session().noise()), (2, 2));
        h.press(&["n"; 9]);
        assert_eq!(h.app.config.noise, 5);
        h.press(&["N"; 9]);
        assert_eq!(h.app.config.noise, 0);
        assert_eq!(crate::state::load_config(&h.state_dir()).noise, 0);
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

    fn panel(h: &Harness) -> String {
        h.app
            .panel_text()
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn escape_toggles_a_side_panel_with_keys_and_files() {
        let mut h = shelf();
        h.press(&["escape"]);
        assert!(h.app.panel);
        let text = panel(&h);
        assert!(text.contains("page down") && text.contains("hide / unhide"));
        assert!(
            text.contains("alpha.txt") && text.contains("> beta.txt") && text.contains("gamma.txt")
        );
        assert!(!text.contains("cover.jpg") && !text.contains("notes"));
        assert!(h.screen().contains("page down")); // drawn over the text
        h.press(&["escape"]);
        assert!(!h.app.panel);
    }

    #[test]
    fn backtick_hides_the_panel_too_and_escape_does_not_undo_it() {
        let mut h = shelf();
        h.press(&["escape", "`"]);
        assert!(h.app.panic && !h.app.panel);
        h.press(&["escape"]);
        assert!(h.app.panic && !h.app.panel);
    }

    #[test]
    fn o_and_shift_o_walk_the_folder_and_keep_each_place() {
        let mut h = shelf();
        h.press(&["space"]);
        let beta_pos = h.session().pos();
        h.press(&["o"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("gamma")
        );
        h.press(&["o"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("alpha")
        );
        h.press(&["O", "O"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("beta")
        );
        assert_eq!(h.session().pos(), beta_pos);
        assert_eq!(load_progress(&h.state_dir()).len(), 3);
    }

    #[test]
    fn digits_open_a_listed_file_only_while_the_panel_is_open() {
        let mut h = shelf();
        h.press(&["1"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("beta")
        );
        h.press(&["escape", "1"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("alpha")
        );
        assert!(panel(&h).contains("> alpha.txt"));
    }

    #[test]
    fn switching_files_is_ignored_while_hidden() {
        let mut h = shelf();
        h.press(&["`", "o"]);
        h.press(&["`"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("beta")
        );
    }

    #[test]
    fn menu_changes_settings_and_saves_them() {
        let mut h = Harness::sample(100, 30);
        h.press(&[":"]);
        assert!(h.app.menu.is_some());
        let screen = h.screen().to_lowercase();
        assert!(screen.contains("fcc config") && screen.contains("target: < code >"));
        for w in ["book", "reader", "okuyucu"] {
            assert!(!screen.contains(w));
        }
        for name in ["target", "lang", "rate", "verbosity", "on hide"] {
            h.app.menu.as_mut().unwrap().focus(name);
            h.press(&["right"]);
        }
        h.press(&["enter"]);
        assert!(h.app.menu.is_none());
        assert_eq!(h.session().mask().name(), "log");
        assert_eq!(h.session().noise(), 1);
        let saved = crate::state::load_config(&h.state_dir());
        assert_eq!(
            (
                saved.mask.as_str(),
                saved.lang.as_str(),
                saved.noise,
                saved.wpm,
                saved.hide.as_str()
            ),
            ("log", "typescript", 1, 240, "shell")
        );
    }

    #[test]
    fn menu_lists_sections_and_jumps_to_one() {
        let mut h = sections_harness();
        h.press(&[":"]);
        let menu = h.app.menu.as_mut().unwrap();
        assert_eq!(menu.sections().len(), 3);
        assert!(menu.sections()[2].0.contains("Kısım 2"));
        menu.focus("sections");
        h.press(&["down", "down", "enter"]);
        assert!(h.app.menu.is_none());
        assert_eq!(h.session().pos(), (2, 0));
    }

    #[test]
    fn menu_escape_closes_backtick_hides_and_marks_jump() {
        let mut h = Harness::sample(100, 30);
        h.press(&["space", "space", "b", "u", "u"]);
        let mark = h.app.bookmarks[0];
        h.press(&[":"]);
        h.app.menu.as_mut().unwrap().focus("marks");
        h.press(&["enter"]);
        assert_eq!(h.session().pos(), (mark[0], mark[1]));
        h.press(&[":", "escape"]);
        assert!(h.app.menu.is_none() && !h.app.panic);
        h.press(&[":", "`"]);
        assert!(h.app.menu.is_none() && h.app.panic);
    }

    #[test]
    fn menu_path_opens_a_file_and_a_bad_one_hides_its_name() {
        let mut h = shelf();
        let alpha = h.dir.join("shelf").join("alpha.txt");
        h.press(&[":"]);
        type_text(&mut h, alpha.to_str().unwrap());
        h.press(&["enter"]);
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("alpha")
        );
        let missing = h.dir.join("Suç ve Ceza.epub");
        h.press(&[":"]);
        type_text(&mut h, missing.to_str().unwrap());
        h.press(&["enter"]);
        let text = h
            .visible()
            .iter()
            .map(Line::plain)
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase()
            + &h.status();
        assert!(text.contains("error:"));
        for part in ["suç", "suc", "ceza", "epub"] {
            assert!(!text.contains(part), "{part}");
        }
        assert!(
            h.session()
                .book
                .paragraph((0, 0))
                .unwrap()
                .text
                .starts_with("alpha")
        ); // still open
    }

    // -- themes --

    #[test]
    fn t_cycles_themes_and_saves_the_choice() {
        let mut h = Harness::sample(80, 24);
        assert_eq!(h.app.config.theme, "dark");
        h.press(&["t"]);
        assert_eq!(h.app.config.theme, "light");
        assert_eq!(crate::state::load_config(&h.state_dir()).theme, "light");
        h.press(&["t"; 5]);
        assert_eq!(h.app.config.theme, "dark"); // round the six themes
        h.press(&["`", "t"]);
        assert_eq!(h.app.config.theme, "dark"); // not while hidden
    }

    #[test]
    fn menu_sets_the_theme() {
        let mut h = Harness::sample(100, 30);
        h.press(&[":"]);
        assert!(h.screen().contains("theme: < dark >"));
        h.app.menu.as_mut().unwrap().focus("theme");
        h.press(&["right", "right", "enter"]);
        assert_eq!(h.app.config.theme, "dark-daltonized");
        assert_eq!(
            crate::state::load_config(&h.state_dir()).theme,
            "dark-daltonized"
        );
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
