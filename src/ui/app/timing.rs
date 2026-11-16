//! Time: deadlines, the stream timer and flash messages.

use super::*;
use crate::pace::line_delay;
use crate::ui::feed::text_chars;
use crate::ui::timer::Clock;

impl App {
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

    pub(super) fn restart_timer(&mut self) {
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

    pub(super) fn timer_fired(&mut self) {
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
}
