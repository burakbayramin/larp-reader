//! Views, filler, themes, files, the menu, search, bookmarks and reading speed.

use super::*;
use crate::masks::{LANGS, MASK_NAMES};
use crate::noise::MAX_NOISE;
use crate::state::save_config;
use crate::ui::menu::section_label;
use crate::ui::theme::THEMES;

impl App {
    /// Switch the disguise in place, staying on the current paragraph.
    pub(super) fn use_mask(&mut self, name: &str, lang: &str) {
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

    pub(super) fn current_mask_name(&self) -> String {
        match &self.session {
            Some(s) => s.mask().name().to_string(),
            None => self
                .spare_mask
                .as_ref()
                .map(|m| m.name().to_string())
                .unwrap_or_else(|| self.config.mask.clone()),
        }
    }

    pub(super) fn cycle_mask(&mut self, step: isize) {
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
    pub(super) fn cycle_lang(&mut self, step: isize) {
        let i = LANGS
            .iter()
            .position(|l| *l == self.config.lang)
            .map(|i| i as isize)
            .unwrap_or(-1);
        let next = LANGS[(i + step).rem_euclid(LANGS.len() as isize) as usize];
        self.use_mask("code", next);
    }

    pub(super) fn use_theme(&mut self, name: &str) {
        self.theme = Theme::new(name, self.theme.depth());
        self.config.theme = self.theme.name().to_string();
    }

    pub(super) fn cycle_theme(&mut self) {
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

    pub(super) fn change_noise(&mut self, delta: i32) {
        let level = (self.config.noise as i32 + delta).clamp(0, MAX_NOISE as i32) as u8;
        self.config.noise = level;
        if let Some(s) = self.session.as_mut() {
            s.set_noise(level);
        }
        self.persist_config();
        self.redraw();
    }

    pub(super) fn cycle_input(&mut self, step: isize) {
        if self.panic {
            return;
        }
        if let Some(next) = panel::cycle_target(self.entry_path.as_deref(), step) {
            self.open_from_menu(&next);
        }
    }

    pub(super) fn open_listed(&mut self, n: usize) {
        if self.panic || !self.panel {
            return;
        }
        if let Some(target) = panel::listed_target(self.entry_path.as_deref(), n) {
            self.open_from_menu(&target);
        }
    }

    /// Open another file, keeping this one's place saved first.
    pub(super) fn open_from_menu(&mut self, path: &Path) {
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

    pub(super) fn open_menu(&mut self) {
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
    pub(super) fn open_report(&mut self) {
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

    pub(super) fn today(&self) -> i64 {
        self.day_override.unwrap_or_else(local_today)
    }

    pub(super) fn on_menu_result(&mut self, result: MenuResult) {
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

    pub(super) fn find(&mut self, start: Pos, backward: bool, wrapped: bool) {
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

    pub(super) fn find_next(&mut self, backward: bool) {
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

    pub(super) fn bookmark(&mut self) {
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

    pub(super) fn jump_mark(&mut self, forward: bool) {
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

    pub(super) fn toggle_auto(&mut self) {
        if self.session.is_none() || self.panic {
            return;
        }
        self.auto = !self.auto;
        self.notice.clear();
        self.redraw();
        self.restart_timer();
    }

    pub(super) fn set_wpm(&mut self, wpm: u32) {
        self.config.wpm = wpm;
        self.persist_config();
        if !self.panic {
            self.flash(&format!("throttle {wpm}/min"), self.now);
        }
        self.restart_timer();
    }

    pub(super) fn persist_config(&self) {
        let _ = save_config(&self.state_dir, &self.config); // best effort: the choice still applies now
    }

    /// The screen that ends just above this one (or the first screen).
    pub(super) fn page_back(&mut self) {
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

    pub(super) fn quit_and_save(&mut self) {
        self.pacer.forget_view(self.now); // the screen being read counts too
        self.try_save();
        self.quit = true;
    }
}
