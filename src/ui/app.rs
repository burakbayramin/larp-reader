//! The interactive reader (`ui/app.py`), as a state machine the terminal loop drives.
//!
//! Keys, timer deadlines and resizes come in through `handle_key`, `on_time`
//! and `resize`; `draw` paints the current state. Time is always passed in, so
//! tests can drive the app without a terminal and without waiting.

mod draw;
mod keys;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::SeedableRng;

use super::feed::{Feed, text_chars};
use super::input::LineInput;
use super::menu::{Menu, MenuResult, section_label};
use super::panel;
use super::reply::Reply;
use super::report::Report;
use super::search::{self, Search};
use super::shell::shell_prompt;
use super::theme::{THEMES, Theme};
use super::timer::{Clock, Pacer};
use crate::book::{Book, Pos};
use crate::errors::FccError;
use crate::loaders::load_book;
use crate::masks::base::{error_lines, plain_line};
use crate::masks::time::{civil, local_today};
use crate::masks::{LANGS, Line, Look, MASK_NAMES, Mask, Rng, Role, get_mask, seeded};
use crate::noise::MAX_NOISE;
use crate::pace::line_delay;
use crate::session::Session;
use crate::state::{
    Config, Entry, last_entry, load_progress, load_stats, save_config, save_progress, save_stats,
};
use crate::stats::Stats;

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
    /// Where the open text was opened (or resumed): the view's header shows
    /// there too, as a session greets you whenever it starts.
    opened_at: Option<crate::book::Pos>,
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
    /// The input box's line while it takes the keys (views with a box only).
    pub ask: Option<LineInput>,
    /// A fake reply to a submitted request, while it plays.
    pub reply: Option<Reply>,
    /// The search bar and the last query.
    pub search: Search,
    /// The settings screen, while it is up.
    pub menu: Option<Menu>,
    /// The side panel with the keys and the folder's files.
    pub panel: bool,
    /// The colour theme and depth everything is drawn in.
    pub theme: Theme,
    /// Words and time read, per day and per text; saved with the progress.
    pub stats: Stats,
    stats_dirty: bool,
    /// A fixed local day instead of the calendar's (tests).
    pub day_override: Option<i64>,
    /// The statistics screen, while it is up.
    pub report: Option<Report>,
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
            opened_at: None,
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
            ask: None,
            reply: None,
            search: Search::default(),
            menu: None,
            panel: false,
            theme,
            stats: Stats::default(),
            stats_dirty: false,
            day_override: None,
            report: None,
        };
        app.stats = load_stats(&app.state_dir);
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
        self.opened_at = Some(pos);
        Ok(())
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
        self.pacer.forget_view(self.now);
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

    /// Open the statistics screen: what was read so far, and the time left.
    fn open_report(&mut self) {
        if self.panic {
            return;
        }
        self.pacer.forget_view(self.now); // the report is not reading time
        self.bank_reading();
        self.panel = false;
        let day = self.today();
        let remaining = self.session.as_ref().map(|s| {
            let from = s.book.index_of(s.pos());
            s.book
                .chapters
                .iter()
                .flat_map(|c| &c.paragraphs)
                .skip(from)
                .map(|p| p.text.split_whitespace().count() as u64)
                .sum()
        });
        self.report = Some(Report::new(&self.stats, day, self.config.wpm, remaining));
    }

    fn today(&self) -> i64 {
        self.day_override.unwrap_or_else(local_today)
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
        self.pacer.forget_view(self.now); // the screen being read counts too
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
        if let Some(r) = &self.reply {
            at = at.min(r.due());
        }
        at
    }

    pub fn on_time(&mut self, now: Instant) {
        self.now = now;
        self.pacer.type_on(now, self.panic, &mut self.feed);
        if self.pacer.is_due(now) {
            self.timer_fired();
        }
        self.play_reply();
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
        } else if self.auto && self.session.is_some() && !self.busy() {
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
        if !self.auto || self.busy() {
            return;
        }
        let Some(s) = self.session.as_mut() else {
            return;
        };
        // the text on screen had its reading time: it was read
        let (shown, id) = (self.pacer.shown_words(), s.book.id.clone());
        if shown > 0 && self.report.is_none() {
            let secs = line_delay(shown, self.config.wpm);
            let day = self.day_override.unwrap_or_else(local_today);
            self.stats.record(day, &id, shown, secs);
            self.stats_dirty = true;
        }
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

    /// Take the screens read since last time into the statistics.
    fn bank_reading(&mut self) {
        let read = self.pacer.take_read();
        if read.is_empty() {
            return;
        }
        let day = self.day_override.unwrap_or_else(local_today);
        for (book, words, seconds) in read {
            self.stats.record(day, &book, words, seconds);
        }
        self.stats_dirty = true;
    }

    fn save_stats(&mut self) -> std::io::Result<()> {
        if !self.stats_dirty {
            return Ok(());
        }
        save_stats(&self.state_dir, &self.stats)?;
        self.stats_dirty = false;
        Ok(())
    }

    fn try_save(&mut self) {
        self.bank_reading();
        if let Err(e) = self.save().and_then(|()| self.save_stats()) {
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
