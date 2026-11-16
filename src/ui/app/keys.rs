//! Keys: what each one does, the input box, replies, hiding and the fake shell.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::pace::step_wpm;
use crate::ui::input::InputEvent;
use crate::ui::shell::shell_output;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) {
        self.now = now;
        self.pacer.finish_typing(&mut self.feed); // any key, paging and hiding included, shows the whole line
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.keys.is_panic(&key) {
            self.toggle_panic(); // works from anywhere, bars and menus included
            return;
        }
        if ctrl && key.code == KeyCode::Char('q') {
            self.quit_and_save();
            return;
        }
        if self.report.take().is_some() {
            self.redraw(); // back to the text: its reading time starts over
            return; // any key closes the report
        }
        if let Some(field) = self.shell.as_mut() {
            if let InputEvent::Submit(command) = field.handle(key) {
                self.run_shell(&command);
            }
            return; // a shell takes every key: nothing here moves the text
        }
        if let Some(field) = self.ask.as_mut() {
            match key.code {
                KeyCode::Esc => self.close_box(),
                _ => {
                    if let InputEvent::Submit(request) = field.handle(key) {
                        self.submit(&request);
                    }
                }
            }
            return; // the box takes every key while it is focused
        }
        if self.reply.is_some() {
            self.end_reply(); // any key: back to the text first, then the key does its own thing
        }
        if let Some(menu) = self.menu.as_mut() {
            if key.code == KeyCode::Char('`') {
                return; // the menu's own hide key; with the panic key moved it does nothing
            }
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
        if let KeyCode::Char(c @ '1'..='9') = key.code {
            self.open_listed(c as usize - '0' as usize); // fixed: the panel numbers them
            return;
        }
        let Some(action) = self.keys.action(&key) else {
            return;
        };
        match action {
            Action::Quit => self.quit_and_save(),
            Action::PageDown => self.page(),
            Action::PageUp => self.page_back(),
            Action::LineDown => self.step(1),
            Action::LineUp => self.step(-1),
            Action::Auto => self.toggle_auto(),
            Action::Faster => self.set_wpm(step_wpm(self.config.wpm, 1)),
            Action::Slower => self.set_wpm(step_wpm(self.config.wpm, -1)),
            Action::NextChapter => self.chapter(1),
            Action::PrevChapter => self.chapter(-1),
            Action::Search => self.open_search(),
            Action::FindNext => self.find_next(false),
            Action::FindPrev => self.find_next(true),
            Action::Bookmark => self.bookmark(),
            Action::NextMask => self.cycle_mask(1),
            Action::PrevMask => self.cycle_mask(-1),
            Action::NextLang => self.cycle_lang(1),
            Action::PrevLang => self.cycle_lang(-1),
            Action::MoreNoise => self.change_noise(1),
            Action::LessNoise => self.change_noise(-1),
            Action::NextFile => self.cycle_input(1),
            Action::PrevFile => self.cycle_input(-1),
            Action::Panel if !self.panic => self.panel = !self.panel,
            Action::Panel | Action::Panic => {} // the panic key was handled first
            Action::Menu => self.open_menu(),
            Action::Theme => self.cycle_theme(),
            Action::Input => self.focus_box(),
            Action::Stats => self.open_report(),
            Action::NextMark => self.jump_mark(true),
            Action::PrevMark => self.jump_mark(false),
        }
    }

    pub(super) fn step(&mut self, n: isize) {
        self.pacer.set_line_step(true);
        self.scroll(n);
        self.pacer.set_line_step(false); // in case nothing was redrawn
    }

    /// Move the manual view by `n` rendered lines; never past the last screen.
    pub(super) fn scroll(&mut self, n: isize) {
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
    pub(super) fn page(&mut self) {
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

    /// Start typing into the input box, in views that draw one. Auto scroll
    /// waits meanwhile, and the screen stops counting as reading time.
    pub(super) fn focus_box(&mut self) {
        if self.session.is_none() || self.prompt_box().is_none() {
            return; // also while hidden: the box is not drawn then
        }
        self.panel = false;
        self.pacer.stop();
        self.pacer.forget_view(self.now); // typing is not reading time
        self.bank_reading();
        self.ask = Some(LineInput::default());
    }

    /// Back to reading where it was: the same screen, auto scroll going again.
    pub(super) fn close_box(&mut self) {
        self.ask = None;
        if !self.auto {
            self.redraw(); // the same lines; only the reading clock starts over
        }
        self.restart_timer();
    }

    /// Play the view's reply to `request` under the screen, scrolling it up
    /// like a terminal; an empty request just closes the box.
    pub(super) fn submit(&mut self, request: &str) {
        self.ask = None;
        let groups = match self.session.as_ref() {
            Some(s) if !request.trim().is_empty() => s.mask().reply(request, self.width()),
            _ => vec![],
        };
        if groups.is_empty() {
            self.close_box();
            return;
        }
        self.feed.set_bottom_anchored(true);
        self.hide_text_above();
        let last = self.feed.visible_lines().last();
        if last.is_some_and(|l| !l.plain().trim().is_empty()) {
            self.feed.append(vec![plain_line("", FG)]); // a gap above the request
        }
        self.reply = Some(Reply::new(groups, self.now));
        self.play_reply();
    }

    /// While a reply plays, nothing of the text stays on screen: the rows
    /// above it become the session's own earlier work (the view's filler),
    /// so it reads as one continuous session. The text comes back after.
    pub(super) fn hide_text_above(&mut self) {
        let rows = self.feed.visible_lines().len();
        let width = self.width();
        let Some(s) = self.session.as_mut() else {
            return;
        };
        let mut rng = seeded(&format!("{}:before-reply", s.book.id));
        let filler = s
            .mask_mut()
            .noise_block(rows, &mut rng, width)
            .unwrap_or_else(|| (0..rows).map(|_| plain_line("", FG)).collect());
        self.feed.set_lines(filler);
    }

    /// Show the reply's lines due now; once it is over, back to the text.
    pub(super) fn play_reply(&mut self) {
        let Some(reply) = self.reply.as_mut() else {
            return;
        };
        let (lines, over) = reply.take_due(self.now);
        self.feed.append(lines);
        if over {
            self.end_reply();
        }
    }

    /// The text again exactly where it was, and auto scroll going on.
    pub(super) fn end_reply(&mut self) {
        self.reply = None;
        self.redraw();
        self.restart_timer();
    }

    /// The box has the keys or a reply plays: auto scroll waits.
    pub(super) fn busy(&self) -> bool {
        self.ask.is_some() || self.reply.is_some()
    }

    pub fn toggle_panic(&mut self) {
        self.panic = !self.panic;
        self.ask = None; // the typed text goes with everything else
        self.reply = None;
        self.pacer.stop();
        self.pacer.finish_typing(&mut self.feed);
        self.feed.clear();
        self.pacer.clear_queue();
        self.pacer.forget_view(self.now); // time spent hidden is not reading time
        self.bank_reading();
        self.search.close();
        self.menu = None;
        self.report = None;
        self.panel = false;
        let shell = self.config.hide == "shell";
        self.feed.set_height(self.view_height()); // the input box comes and goes with hiding
        if self.panic {
            self.feed.set_bottom_anchored(true);
            let (width, rows) = (self.width(), self.view_height() - usize::from(shell));
            let mut rng = self.panic_rng.clone();
            self.mask_mut().stream();
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
    pub(super) fn run_shell(&mut self, command: &str) {
        let command = command.trim().to_string();
        let width = self.width();
        let mut rng = self.panic_rng.clone();
        self.mask_mut().stream();
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
    pub(super) fn go(&mut self, pos: Pos) {
        if let Some(s) = self.session.as_mut() {
            s.set_pos(pos);
        }
        self.notice.clear();
        self.redraw();
    }

    pub(super) fn chapter(&mut self, step: i32) {
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

    pub(super) fn open_search(&mut self) {
        if self.session.is_some() && !self.panic {
            self.panel = false;
            self.search.open();
        }
    }
}
