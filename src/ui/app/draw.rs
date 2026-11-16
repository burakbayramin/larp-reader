//! Drawing: the screen laid out from the session, the status line and the frame.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

use super::*;
use crate::masks::palette::YELLOW;
use crate::text::truncate_cells;
use crate::ui::report::compact;

/// The end of `line` that fits in `cols` cells: an input bar scrolls sideways
/// like a terminal, so the typed text and the cursor stay in view.
pub(super) fn tail_fitting(line: &str, cols: usize) -> String {
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

impl App {
    pub(super) fn width(&self) -> usize {
        self.cols.max(1)
    }

    pub(super) fn view_height(&self) -> usize {
        let input = if self.prompt_box().is_some() {
            BOX_ROWS
        } else {
            0
        };
        self.rows.saturating_sub(1 + input).max(1)
    }

    /// The input box's placeholder, when the box is drawn: only for views that
    /// have one, never while hidden, and not in small windows.
    pub(super) fn prompt_box(&self) -> Option<&'static str> {
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
        self.feed
            .set_bottom_anchored(self.auto || self.reply.is_some());
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
            let start = s.book.index_of(s.pos()) == 0 || self.opened_at == Some(s.pos());
            let at_top = start && (self.auto || s.line_offset == 0);
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
            self.bank_reading();
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
        let mut right = s
            .mask()
            .status_right(done, total, self.pacer.elapsed(self.now));
        let left_w = text.width();
        let width = |parts: &[crate::masks::Segment]| -> usize {
            parts.iter().map(|(t, _)| t.width()).sum()
        };
        // today's reading goes last, and only where it still fits
        let today = self.stats.day(self.today()).words;
        if today > 0 {
            let mut more = vec![(format!("today {} words", compact(today)), STATUS_FG)];
            if !right.is_empty() {
                more.insert(0, ("  ·  ".to_string(), STATUS_FG));
            }
            if left_w + 2 + width(&right) + width(&more) < self.cols {
                right.extend(more);
            }
        }
        let right_w = width(&right);
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
            || self.report.is_some()
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
    pub(super) fn with_sprite(
        &self,
        mut text: ratatui::text::Text<'static>,
    ) -> ratatui::text::Text<'static> {
        let Some(s) = self.session.as_ref().filter(|_| !self.panic) else {
            return text;
        };
        let Some(rows) = s.mask().sprite(
            self.pacer.elapsed(self.now),
            self.auto || self.reply.is_some(),
        ) else {
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
        if let Some(report) = &self.report {
            report.draw(&self.current_mask_name(), frame, theme);
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
            const LEAD: &str = " > ";
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(style(BOX_BORDER));
            let (text, look) = match &self.ask {
                // the typed text scrolls sideways, keeping a cell for the cursor
                Some(f) => {
                    let room = (input_area.width as usize).saturating_sub(2 + LEAD.len() + 1);
                    (tail_fitting(&f.value, room), FG)
                }
                // empty while it "works", like a session that is busy
                None if self.reply.is_some() => (String::new(), STATUS_FG),
                None => (placeholder.to_string(), STATUS_FG),
            };
            let typed = unicode_width::UnicodeWidthStr::width(text.as_str()) as u16;
            let prompt = TLine::from(vec![
                Span::styled(LEAD, style(FG)),
                Span::styled(text, style(look)),
            ]);
            frame.render_widget(Paragraph::new(prompt).block(block), input_area);
            if self.ask.is_some() {
                let x = input_area.x + 1 + LEAD.len() as u16 + typed;
                frame.set_cursor_position((x, input_area.y + 1));
            }
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
            panel::draw_panel(frame, theme, self.entry_path.as_deref(), &self.keys);
        }
    }

    /// The side panel's text: the keys, then the folder's openable files.
    pub fn panel_text(&self, theme: &Theme) -> Vec<ratatui::text::Line<'static>> {
        panel::panel_text(theme, self.entry_path.as_deref(), &self.keys)
    }

    /// The mask in use: the open session's, or the one kept for errors.
    pub(super) fn mask_mut(&mut self) -> &mut dyn Mask {
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
        let box_gone = self.ask.is_some() && self.prompt_box().is_none();
        if box_gone {
            self.ask = None; // the box is gone in a small window
        }
        // a reply laid out for the old width ends: the text comes back re-wrapped
        let closed = box_gone | self.reply.take().is_some();
        let width = self.width();
        if let Some(s) = self.session.as_mut()
            && s.width() != width
        {
            s.set_width(width);
            let pos = s.pos();
            s.set_pos(pos); // re-wrapped: restart at the paragraph's top
        }
        self.redraw();
        if closed {
            self.restart_timer();
        }
    }
}
