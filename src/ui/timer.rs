//! What happens on the clock: the stream's next line (auto scroll and the
//! hidden screen), the reading rate learnt from paging, the typewriter that
//! reveals a streamed line, and the clock animated status labels count from.
//!
//! `Pacer` keeps the state; `App` hands it the session, the feed and the
//! settings it needs, and decides what each step means for the screen.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::feed::Feed;
use crate::book::Pos;
use crate::masks::Line;
use crate::pace::{learn, line_delay, words_in};
use crate::session::Session;

/// Filler lines per second while hidden.
pub const PANIC_RATE: f64 = 4.5;
/// Redraw interval while the status label animates.
pub const ANIMATION_FRAME: Duration = Duration::from_millis(120);
/// Redraw interval while a line types out.
pub const TYPING_FRAME: Duration = Duration::from_millis(30);
/// Typing speed of a streamed line, in characters per second.
pub const TYPING_RATE: f64 = 180.0;
/// A line is fully typed within this share of its reading time.
pub const TYPING_SHARE: f64 = 0.4;

/// What the stream timer runs for.
pub enum Clock {
    /// The hidden screen: filler on a timer, unless a shell waits for input.
    Hidden { shell: bool },
    /// Auto scroll with a text open.
    Auto,
    /// Nothing streams.
    Idle,
}

/// A manual screen's identity: the text, its paragraph and the line offset in it.
pub type ViewKey = (String, Pos, usize);

pub struct Pacer {
    /// When the stream shows its next line.
    next_tick: Option<Instant>,
    /// Auto mode: lines of the paragraph being streamed, not yet shown.
    queue: VecDeque<Line>,
    /// Words of the newest text on screen, still to be read.
    last_words: usize,
    /// Reading pace: which manual screen is up, since when, and the text lines seen on it.
    view_key: Option<ViewKey>,
    view_since: Instant,
    view_seen: HashMap<(Option<Pos>, String), usize>,
    pub view_words: usize,
    /// The next redraw only scrolled by a line: keep timing the screen.
    line_step: bool,
    /// When the current disguise came up: animated status labels count from here.
    started: Instant,
    /// The newest streamed line typing out: since when, for how long, its characters.
    typing: Option<(Instant, Duration, usize)>,
}

impl Pacer {
    pub fn new(now: Instant) -> Pacer {
        Pacer {
            next_tick: None,
            queue: VecDeque::new(),
            last_words: 0,
            view_key: None,
            view_since: now,
            view_seen: HashMap::new(),
            view_words: 0,
            line_step: false,
            started: now,
            typing: None,
        }
    }

    // -- the stream timer ------------------------------------------------------

    /// When the stream's next step is due, if it runs.
    pub fn due(&self) -> Option<Instant> {
        self.next_tick
    }

    pub fn is_due(&self, now: Instant) -> bool {
        self.next_tick.is_some_and(|t| now >= t)
    }

    pub fn stop(&mut self) {
        self.next_tick = None;
    }

    /// Start the timer over from `now` for `clock`.
    pub fn restart(&mut self, clock: Clock, now: Instant, wpm: u32) {
        self.stop();
        match clock {
            Clock::Hidden { shell: false } => {
                self.next_tick = Some(now + Duration::from_secs_f64(1.0 / PANIC_RATE));
            }
            Clock::Hidden { shell: true } => {} // a shell waits for input
            // the text already on screen gets its reading time before the next line
            Clock::Auto => self.schedule_line(now, wpm),
            Clock::Idle => {}
        }
    }

    /// The next line comes once the newest text has had its reading time.
    pub fn schedule_line(&mut self, now: Instant, wpm: u32) {
        self.next_tick = Some(now + Duration::from_secs_f64(line_delay(self.last_words, wpm)));
    }

    /// The soonest redraw the clock needs: the next stream step, the next frame
    /// of an animated label (when `animating`), or of a line typing out.
    pub fn next_deadline(&self, now: Instant, animating: bool) -> Option<Instant> {
        let mut at = self.next_tick;
        let mut soonest = |t: Instant| at = Some(at.map_or(t, |a| a.min(t)));
        if animating {
            soonest(now + ANIMATION_FRAME);
        }
        if self.typing.is_some() {
            soonest(now + TYPING_FRAME);
        }
        at
    }

    // -- the auto stream -------------------------------------------------------

    /// A fresh layout: lines still queued from the old one are dropped.
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Auto mode laid out `para` as the newest text on screen.
    pub fn show_paragraph(&mut self, para: &[Line]) {
        self.last_words = words_in(para);
    }

    /// One stream step from `s`: the filler before the next text line, and
    /// that line. True as well once the text has run out.
    pub fn next_lines(&mut self, s: &mut Session) -> (Vec<Line>, bool) {
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
        (out, finished)
    }

    // -- the reading rate --------------------------------------------------------

    /// The coming redraw only scrolls by a line (or the step is over).
    pub fn set_line_step(&mut self, on: bool) {
        self.line_step = on;
    }

    /// Time a manual screen from when it first shows.
    ///
    /// Scrolling by a line keeps the clock running and adds the lines it brings
    /// in, so a page turn afterwards still divides everything read by the time
    /// it really took.
    pub fn watch(&mut self, key: ViewKey, shown: &[Line], now: Instant) {
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
            self.view_since = now;
            self.view_seen = seen;
        }
        self.view_key = Some(key);
        self.view_words = self.view_seen.values().sum();
    }

    /// Stop timing the screen: time spent elsewhere (hidden) is not reading time.
    pub fn forget_view(&mut self) {
        self.view_key = None;
    }

    /// The reading rate the screen up since `view_since` suggests, read by `now`.
    pub fn learned_rate(&self, wpm: u32, now: Instant) -> Option<u32> {
        let read = now.saturating_duration_since(self.view_since).as_secs_f64();
        learn(wpm, self.view_words, read)
    }

    // -- the status clock ----------------------------------------------------------

    /// A new disguise came up: its clock starts at zero.
    pub fn restart_clock(&mut self, now: Instant) {
        self.started = now;
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        now.saturating_duration_since(self.started)
    }

    // -- the typewriter --------------------------------------------------------------

    pub fn typing(&self) -> bool {
        self.typing.is_some()
    }

    /// Type the newest line out, done well before its reading time is up.
    pub fn start_typing(&mut self, chars: usize, now: Instant, wpm: u32, feed: &mut Feed) {
        let reading = line_delay(self.last_words, wpm) * TYPING_SHARE;
        let span = (chars as f64 / TYPING_RATE).min(reading);
        self.typing = Some((now, Duration::from_secs_f64(span), chars));
        feed.set_reveal(Some(0));
    }

    /// Show the characters due by `now`; all of them at once while `hidden`.
    pub fn type_on(&mut self, now: Instant, hidden: bool, feed: &mut Feed) {
        let Some((since, span, chars)) = self.typing else {
            return;
        };
        let gone = now.saturating_duration_since(since).as_secs_f64();
        let span = span.as_secs_f64();
        if hidden || gone >= span {
            self.finish_typing(feed);
            return;
        }
        let shown = ((chars as f64 * gone / span).ceil() as usize).min(chars);
        feed.set_reveal(Some(shown));
    }

    /// The line being typed shows in full at once.
    pub fn finish_typing(&mut self, feed: &mut Feed) {
        if self.typing.take().is_some() {
            feed.set_reveal(None);
        }
    }
}
