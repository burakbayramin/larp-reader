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

use rand::SeedableRng;

use super::feed::{Feed, text_chars};
use super::input::{InputEvent, LineInput};
use super::menu::{Menu, MenuResult, section_label};
use super::panel;
use super::search::{self, Search};
use super::shell::{shell_output, shell_prompt};
use super::theme::{THEMES, Theme};
use super::timer::{Clock, Pacer};
use crate::book::{Book, Pos};
use crate::errors::FccError;
use crate::loaders::load_book;
use crate::masks::base::{error_lines, plain_line};
use crate::masks::palette::YELLOW;
use crate::masks::time::civil;
use crate::masks::{LANGS, Line, Look, MASK_NAMES, Mask, Rng, Role, get_mask, seeded};
use crate::noise::MAX_NOISE;
use crate::pace::step_wpm;
use crate::session::Session;
use crate::state::{Config, Entry, last_entry, load_progress, save_config, save_progress};
use crate::text::truncate_cells;

pub const MIN_WIDTH: usize = 40;
pub const AUTOSAVE: Duration = Duration::from_secs(10);
pub const FLASH: Duration = Duration::from_secs(2);
/// Rows of the input box some views draw above the status line.
pub const BOX_ROWS: usize = 3;
/// Shorter windows keep every row for the text.
const BOX_MIN_ROWS: usize = 10;
pub(crate) const BOX_BORDER: Look = Look::rgb(0x5a5a5a);
const WARNING: &str = "warning: window too narrow";
pub(crate) const STATUS_FG: Look = Look::fg(Role::Gutter);
pub(crate) const FG: Look = Look::fg(Role::Text);
const SAVED_MSG: &str = "cache saved";
const NO_MARKS_MSG: &str = "cache empty";

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
    /// The stream timer, the reading rate, the typewriter and the status clock.
    pub pacer: Pacer,
    /// Filler for the hidden screen: unseeded, it never needs to repeat.
    panic_rng: Rng,
    prompt: String,
    /// The fake shell's input line, while it is up.
    pub shell: Option<LineInput>,
    /// The search bar and the last query.
    pub search: Search,
    /// The settings screen, while it is up.
    pub menu: Option<Menu>,
    /// The side panel with the keys and the folder's files.
    pub panel: bool,
    /// The colour theme and depth everything is drawn in.
    pub theme: Theme,
}

impl App {
    /// `theme` is the configured theme at the terminal's colour depth.
    pub fn new(
        path: Option<PathBuf>,
        config: Config,
        theme: Theme,
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
            now,
            pacer: Pacer::new(now),
            panic_rng: Rng::seed_from_u64(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(1),
            ),
            prompt: shell_prompt(),
            shell: None,
            search: Search::default(),
            menu: None,
            panel: false,
            theme,
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
        let input = if self.prompt_box().is_some() {
            BOX_ROWS
        } else {
            0
        };
        self.rows.saturating_sub(1 + input).max(1)
    }

    /// The input box's placeholder, when the box is drawn: only for views that
    /// have one, never while hidden, and not in small windows.
    fn prompt_box(&self) -> Option<&'static str> {
        if self.panic || self.cols < MIN_WIDTH || self.rows < BOX_MIN_ROWS {
            return None;
        }
        self.session.as_ref().and_then(|s| s.mask().prompt_box())
    }

    /// Lay the screen out again from the session: warning, notices, header and text.
    pub fn redraw(&mut self) {
        self.pacer.finish_typing(&mut self.feed);
        self.feed.set_height(self.view_height());
        if self.panic {
            return;
        }
        self.feed.set_bottom_anchored(self.auto);
        self.pacer.clear_queue();
        let width = self.width();
        let mut lines = vec![];
        if width < MIN_WIDTH {
            lines.push(plain_line(truncate_cells(WARNING, width), YELLOW));
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
                self.pacer.show_paragraph(&para);
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
            self.pacer.watch(key, &shown, self.now);
        }
        self.feed.set_lines(self.search.highlighted(lines));
    }

    pub fn status_text(&self) -> String {
        if let Some((msg, _)) = &self.flash {
            return msg.clone();
        }
        match &self.session {
            Some(s) => {
                let (done, total) = s.progress();
                let elapsed = self.pacer.elapsed(self.now);
                s.mask()
                    .animated_label(done, total, self.width(), elapsed)
                    .unwrap_or_else(|| s.mask().progress_label(done, total, self.width()))
            }
            None => String::new(),
        }
    }

    /// The status line in colour: the label's parts coloured, and the view's
    /// own extras at the right end when the window has room for them. A flash
    /// message stays plain.
    pub fn status_segments(&self) -> Vec<crate::masks::Segment> {
        use unicode_width::UnicodeWidthStr;
        let text = self.status_text();
        let (Some(s), None) = (self.session.as_ref(), self.flash.as_ref()) else {
            return vec![(text, STATUS_FG)];
        };
        let mut out = crate::masks::base::colour_label(&text);
        let (done, total) = s.progress();
        let right = s
            .mask()
            .status_right(done, total, self.pacer.elapsed(self.now));
        let left_w = text.width();
        let right_w: usize = right.iter().map(|(t, _)| t.width()).sum();
        let room = self.cols;
        if !right.is_empty() && left_w + 2 + right_w < room {
            out.push((" ".repeat(room - left_w - right_w - 1), STATUS_FG));
            out.extend(right);
        }
        out
    }

    /// The status line shows a label that moves on its own, so the screen needs
    /// redrawing often. Never while hidden: the hidden screen redraws on its own
    /// timer (or waits at a prompt) and keeps idle use low.
    pub fn animating(&self) -> bool {
        if self.panic
            || self.flash.is_some()
            || self.menu.is_some()
            || self.shell.is_some()
            || self.search.is_open()
        {
            return false;
        }
        self.session.as_ref().is_some_and(|s| {
            s.mask()
                .animated_label(0, 0, self.width(), Duration::ZERO)
                .is_some()
        })
    }

    /// Swap the header figure's rows for the frame due now (blinking, and
    /// stepping while text streams). Never while hidden.
    fn with_sprite(&self, mut text: ratatui::text::Text<'static>) -> ratatui::text::Text<'static> {
        let Some(s) = self.session.as_ref().filter(|_| !self.panic) else {
            return text;
        };
        let Some(rows) = s.mask().sprite(self.pacer.elapsed(self.now), self.auto) else {
            return text;
        };
        for line in &mut text.lines {
            for span in &mut line.spans {
                if let Some((_, now)) = rows.iter().find(|(drawn, _)| span.content == *drawn) {
                    span.content = (*now).into();
                }
            }
        }
        text
    }

    pub fn draw(&self, frame: &mut Frame) {
        self.draw_in(frame, &self.theme);
    }

    /// Draw in `theme` (the app's own, but tests may pick another).
    pub fn draw_in(&self, frame: &mut Frame, theme: &Theme) {
        if let Some(menu) = &self.menu {
            menu.draw(frame, theme);
            return;
        }
        let base = theme.base();
        let style = |look: Look| theme.style(look);
        frame.render_widget(ratatui::widgets::Block::default().style(base), frame.area());
        let input = self.prompt_box();
        let box_rows = if input.is_some() { BOX_ROWS as u16 } else { 0 };
        let [feed, input_area, status] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(box_rows),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(self.with_sprite(self.feed.text(theme))),
            feed,
        );
        if let Some(placeholder) = input {
            use ratatui::text::{Line as TLine, Span};
            use ratatui::widgets::{Block, BorderType, Borders};
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(style(BOX_BORDER));
            let prompt = TLine::from(vec![
                Span::styled(" > ", style(FG)),
                Span::styled(placeholder, style(STATUS_FG)),
            ]);
            frame.render_widget(Paragraph::new(prompt).block(block), input_area);
        }
        let bar = match &self.shell {
            Some(f) => Some(format!("{}{}", self.prompt, f.value)),
            None => self.search.bar_text(),
        };
        if let Some(line) = bar {
            let line = tail_fitting(&line, status.width.saturating_sub(1) as usize);
            let x = status.x + unicode_width::UnicodeWidthStr::width(line.as_str()) as u16;
            frame.render_widget(Paragraph::new(line).style(base.patch(style(FG))), status);
            frame.set_cursor_position((x, status.y));
        } else {
            let line = ratatui::text::Line::from(
                self.status_segments()
                    .into_iter()
                    .map(|(t, look)| ratatui::text::Span::styled(t, style(look)))
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(
                Paragraph::new(line).style(base.patch(style(STATUS_FG))),
                status,
            );
        }
        if self.panel {
            panel::draw_panel(frame, theme, self.entry_path.as_deref());
        }
    }

    /// The side panel's text: the keys, then the folder's openable files.
    pub fn panel_text(&self, theme: &Theme) -> Vec<ratatui::text::Line<'static>> {
        panel::panel_text(theme, self.entry_path.as_deref())
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
        self.pacer.finish_typing(&mut self.feed); // any key, paging and hiding included, shows the whole line
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
        if self.search.is_open() {
            if self.search.handle(key) {
                match self.session.as_ref().map(Session::pos) {
                    Some(pos) if !self.search.query.is_empty() => self.find(pos, false, false),
                    _ => self.redraw(),
                }
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
        self.pacer.set_line_step(true);
        self.scroll(n);
        self.pacer.set_line_step(false); // in case nothing was redrawn
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
            if let Some(rate) = self.pacer.learned_rate(self.config.wpm, self.now)
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
        self.pacer.stop();
        self.pacer.finish_typing(&mut self.feed);
        self.feed.clear();
        self.pacer.clear_queue();
        self.pacer.forget_view(); // time spent hidden is not reading time
        self.search.close();
        self.menu = None;
        self.panel = false;
        let shell = self.config.hide == "shell";
        self.feed.set_height(self.view_height()); // the input box comes and goes with hiding
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
            self.search.open();
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
        self.pacer.restart_clock(self.now); // a new session: its clock starts at zero
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
        self.theme = Theme::new(name, self.theme.depth());
        self.config.theme = self.theme.name().to_string();
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
        if self.panic {
            return;
        }
        if let Some(next) = panel::cycle_target(self.entry_path.as_deref(), step) {
            self.open_from_menu(&next);
        }
    }

    fn open_listed(&mut self, n: usize) {
        if self.panic || !self.panel {
            return;
        }
        if let Some(target) = panel::listed_target(self.entry_path.as_deref(), n) {
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
                self.pacer.clear_queue();
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
        let (target, msg) = self.search.find(&s.book, start, backward, wrapped);
        match target {
            None => self.redraw(),
            Some(pos) => self.go(pos),
        }
        if let Some(msg) = msg {
            self.flash(msg, self.now);
        }
    }

    fn find_next(&mut self, backward: bool) {
        let Some(s) = self.session.as_ref() else {
            return;
        };
        if self.panic || self.search.query.is_empty() {
            return;
        }
        if let Some((start, wrapped)) = search::next_start(&s.book, s.pos(), backward) {
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
        if let Some((_, until)) = &self.flash {
            at = at.min(*until);
        }
        if let Some(t) = self.pacer.next_deadline(self.now, self.animating()) {
            at = at.min(t);
        }
        at
    }

    pub fn on_time(&mut self, now: Instant) {
        self.now = now;
        self.pacer.type_on(now, self.panic, &mut self.feed);
        if self.pacer.is_due(now) {
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

    fn restart_timer(&mut self) {
        let clock = if self.panic {
            Clock::Hidden {
                shell: self.config.hide == "shell",
            }
        } else if self.auto && self.session.is_some() {
            Clock::Auto
        } else {
            Clock::Idle
        };
        self.pacer.restart(clock, self.now, self.config.wpm);
    }

    fn timer_fired(&mut self) {
        self.pacer.stop();
        self.tick();
        if self.panic {
            self.restart_timer();
            return;
        }
        if self.auto && !self.panic && self.pacer.due().is_none() {
            self.pacer.schedule_line(self.now, self.config.wpm);
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
        let (out, finished) = self.pacer.next_lines(s);
        let types = s.mask().typewriter();
        if !out.is_empty() {
            let out = self.search.highlighted(out);
            let chars = out.last().filter(|l| l.is_book).map(text_chars);
            self.feed.append(out);
            if let Some(chars) = chars.filter(|&c| types && c > 0) {
                self.pacer
                    .start_typing(chars, self.now, self.config.wpm, &mut self.feed);
            }
        }
        if finished {
            // end of text: back to manual mode so the paging keys work again
            self.pacer.stop();
            self.auto = false;
            self.redraw();
        }
    }

    pub fn timer_due(&self) -> Option<Instant> {
        self.pacer.due()
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
                self.pacer.finish_typing(&mut self.feed);
                self.feed.append(lines);
            } else {
                self.redraw();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
