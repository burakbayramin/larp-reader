//! The interactive reader (`ui/app.py`), as a state machine the terminal loop drives.
//!
//! Keys, timer deadlines and resizes come in through `handle_key`, `on_time`
//! and `resize`; `draw` paints the current state. Time is always passed in, so
//! tests can drive the app without a terminal and without waiting.

mod commands;
mod draw;
mod keys;
mod save;
pub use save::timestamp;
mod timing;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::SeedableRng;

use super::feed::Feed;
use super::input::LineInput;
use super::menu::{Menu, MenuResult};
use super::panel;
use super::reply::Reply;
use super::report::Report;
use super::search::{self, Search};
use super::shell::shell_prompt;
use super::theme::Theme;
use super::timer::Pacer;
use crate::book::{Book, Pos};
use crate::errors::FccError;
use crate::loaders::load_book;
use crate::masks::base::{error_lines, plain_line};
use crate::masks::time::local_today;
use crate::masks::{Line, Look, Mask, Rng, Role, get_mask, seeded};
use crate::session::Session;
use crate::state::{Config, last_entry, load_progress, load_stats};
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
        let entry = crate::sync::merge(
            &self.config.sync_dir,
            &book.id,
            progress.get(&book.id).cloned(),
        );
        let pos = entry
            .as_ref()
            .map(|e| valid_pos(&book, (e.chapter, e.paragraph)))
            .unwrap_or((0, 0));
        self.bookmarks = entry.map(|e| e.bookmarks).unwrap_or_default();
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
}

#[cfg(test)]
pub(crate) mod tests;
