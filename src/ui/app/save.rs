//! Saving progress and statistics.

use super::*;
use crate::masks::time::civil;
use crate::state::{Entry, save_progress, save_stats};

/// Local-time-free `2026-11-12T10:04:05` (UTC) for the progress file's `updated_at`.
pub fn timestamp() -> String {
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

impl App {
    pub(super) fn save(&self) -> std::io::Result<()> {
        let (Some(s), Some(path)) = (&self.session, &self.entry_path) else {
            return Ok(());
        };
        let mut data = load_progress(&self.state_dir);
        let entry = Entry {
            path: path.clone(),
            chapter: s.pos().0,
            paragraph: s.pos().1,
            bookmarks: self.bookmarks.clone(),
            updated_at: timestamp(),
        };
        crate::sync::push(&self.config.sync_dir, &s.book.id, &entry);
        data.insert(s.book.id.clone(), entry);
        save_progress(&self.state_dir, &data)
    }

    /// Take the screens read since last time into the statistics.
    pub(super) fn bank_reading(&mut self) {
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

    pub(super) fn save_stats(&mut self) -> std::io::Result<()> {
        if !self.stats_dirty {
            return Ok(());
        }
        save_stats(&self.state_dir, &self.stats)?;
        self.stats_dirty = false;
        Ok(())
    }

    pub(super) fn try_save(&mut self) {
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
