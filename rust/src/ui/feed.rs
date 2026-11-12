//! The scrolling text area (`ui/feed.py`): masked lines, shown from the top or, while
//! streaming, from the bottom like a terminal.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line as TLine, Span, Text};
use ratatui::widgets::{Paragraph, Widget};

use super::style::style;
use crate::masks::Line;

/// Lines kept for scrollback while streaming.
pub const MAX_LINES: usize = 2000;

#[derive(Default)]
pub struct Feed {
    lines: Vec<Line>,
    bottom: bool,
    height: usize,
}

impl Feed {
    /// Bottom-anchored feeds show the last rows, like a scrolling terminal.
    pub fn set_bottom_anchored(&mut self, bottom: bool) {
        self.bottom = bottom;
    }

    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
    }

    pub fn set_lines(&mut self, lines: Vec<Line>) {
        self.lines = lines;
    }

    pub fn append(&mut self, lines: Vec<Line>) {
        self.lines.extend(lines);
        if self.lines.len() > MAX_LINES {
            let extra = self.lines.len() - MAX_LINES;
            self.lines.drain(..extra);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn visible_lines(&self) -> &[Line] {
        if self.bottom {
            &self.lines[self.lines.len().saturating_sub(self.height)..]
        } else {
            &self.lines[..self.lines.len().min(self.height)]
        }
    }

    pub fn history_lines(&self) -> &[Line] {
        &self.lines
    }

    pub fn text(&self) -> Text<'static> {
        Text::from(
            self.visible_lines()
                .iter()
                .map(|l| {
                    TLine::from(
                        l.segments
                            .iter()
                            .map(|(t, s)| Span::styled(t.clone(), style(s)))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        )
    }
}

impl Widget for &Feed {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.text()).render(area, buf); // no wrapping: long lines are cut at the edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks::base::plain_line;

    #[test]
    fn top_or_bottom_anchored_and_capped() {
        let mut f = Feed::default();
        f.set_height(2);
        f.set_lines((0..5).map(|i| plain_line(i.to_string(), "")).collect());
        assert_eq!(
            f.visible_lines()
                .iter()
                .map(Line::plain)
                .collect::<Vec<_>>(),
            vec!["0", "1"]
        );
        f.set_bottom_anchored(true);
        assert_eq!(
            f.visible_lines()
                .iter()
                .map(Line::plain)
                .collect::<Vec<_>>(),
            vec!["3", "4"]
        );
        f.append((0..MAX_LINES).map(|_| plain_line("x", "")).collect());
        assert_eq!(f.history_lines().len(), MAX_LINES);
    }
}
