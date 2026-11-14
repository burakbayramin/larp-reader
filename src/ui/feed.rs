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
    /// Typewriter: how many text characters of the newest line are drawn so far.
    /// The whole line is in the history; only drawing holds part of it back.
    reveal: Option<usize>,
}

/// `line` with only the first `shown` characters of its text segments drawn;
/// the rest become blanks of the same width, so backgrounds and gutters stay put.
pub fn partial(line: &Line, shown: usize) -> Line {
    use unicode_width::UnicodeWidthChar;
    let mut left = shown;
    let segments = line
        .segments
        .iter()
        .map(|(text, style)| {
            if !style.split_whitespace().any(|t| t == "book") {
                return (text.clone(), style.clone());
            }
            let mut out = String::new();
            for c in text.chars() {
                if left > 0 {
                    out.push(c);
                    left -= 1;
                } else {
                    out.extend(std::iter::repeat_n(' ', c.width().unwrap_or(0)));
                }
            }
            (out, style.clone())
        })
        .collect();
    Line {
        segments,
        is_book: line.is_book,
        pos: line.pos,
    }
}

/// Characters of `line` that the typewriter types out.
pub fn text_chars(line: &Line) -> usize {
    line.segments
        .iter()
        .filter(|(_, s)| s.split_whitespace().any(|t| t == "book"))
        .map(|(t, _)| t.chars().count())
        .sum()
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
        self.reveal = None;
        self.lines = lines;
    }

    /// Draw only `shown` text characters of the newest line (None: all of it).
    pub fn set_reveal(&mut self, shown: Option<usize>) {
        self.reveal = shown;
    }

    pub fn reveal(&self) -> Option<usize> {
        self.reveal
    }

    /// Adds lines in full: a line still being typed out finishes first.
    pub fn append(&mut self, lines: Vec<Line>) {
        self.reveal = None;
        self.lines.extend(lines);
        if self.lines.len() > MAX_LINES {
            let extra = self.lines.len() - MAX_LINES;
            self.lines.drain(..extra);
        }
    }

    pub fn clear(&mut self) {
        self.reveal = None;
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
        let visible = self.visible_lines();
        let typing = match self.reveal {
            Some(n) if self.bottom && !visible.is_empty() => Some((visible.len() - 1, n)),
            _ => None,
        };
        Text::from(
            visible
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let cut;
                    let l = match typing {
                        Some((last, n)) if i == last => {
                            cut = partial(l, n);
                            &cut
                        }
                        _ => l,
                    };
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

    #[test]
    fn partial_keeps_gutters_and_width_and_hides_the_rest_of_the_text() {
        let l = Line::book(
            vec![
                ("  12 + ".into(), "dim".into()),
                ("héllo wörld".into(), "book fg".into()),
                ("   ".into(), "on red".into()),
            ],
            (0, 0),
        );
        assert_eq!(text_chars(&l), 11);
        let p = partial(&l, 3);
        assert_eq!(p.plain(), "  12 + hél           ");
        assert_eq!(p.plain().chars().count(), l.plain().chars().count());
        assert_eq!(partial(&l, 99).plain(), l.plain());
        let mut f = Feed::default();
        f.set_height(5);
        f.set_bottom_anchored(true);
        f.append(vec![plain_line("x", ""), l.clone()]);
        f.set_reveal(Some(0));
        let shown: Vec<String> = f.text().lines.iter().map(|t| t.to_string()).collect();
        assert_eq!(shown[1].trim(), "12 +");
        assert_eq!(f.history_lines()[1], l); // the history has it all
        f.append(vec![]);
        assert_eq!(f.reveal(), None);
    }
}
