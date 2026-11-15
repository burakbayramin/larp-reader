//! Search: the `/` bar, the last query, and its matches marked in the text.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::input::{InputEvent, LineInput};
use crate::book::{Book, Pos, match_spans};
use crate::masks::Line;

pub const NOT_FOUND_MSG: &str = "Pattern not found";

/// The flash after a search wrapped around the end (forward) or the start (backward).
fn wrap_msg(backward: bool) -> &'static str {
    if backward {
        "search hit TOP, continuing at BOTTOM"
    } else {
        "search hit BOTTOM, continuing at TOP"
    }
}

/// `line` with every match of `query` inside its text segments shown reversed (like grep --color).
pub fn highlight(line: &Line, query: &str) -> Line {
    let mut segments = vec![];
    for (text, look) in &line.segments {
        let spans = if look.book {
            match_spans(text, query)
        } else {
            vec![]
        };
        if spans.is_empty() {
            segments.push((text.clone(), *look));
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        let piece = |a: usize, b: usize| chars[a..b].iter().collect::<String>();
        let mut at = 0;
        for (start, end) in spans {
            if start > at {
                segments.push((piece(at, start), *look));
            }
            segments.push((piece(start, end), look.reverse()));
            at = end;
        }
        if at < chars.len() {
            segments.push((piece(at, chars.len()), *look));
        }
    }
    Line {
        segments,
        is_book: line.is_book,
        pos: line.pos,
    }
}

/// The search bar while it is up, and the last query.
#[derive(Default)]
pub struct Search {
    /// The search input line, while it is up.
    bar: Option<LineInput>,
    /// The last search; its matches are highlighted.
    pub query: String,
}

impl Search {
    pub fn is_open(&self) -> bool {
        self.bar.is_some()
    }

    pub fn open(&mut self) {
        self.bar = Some(LineInput::default());
    }

    pub fn close(&mut self) {
        self.bar = None;
    }

    /// Give a key to the open bar. True when a query was submitted: the bar
    /// is gone and `query` holds the new search.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let Some(field) = self.bar.as_mut() else {
            return false;
        };
        match field.handle(key) {
            InputEvent::Submit(query) => {
                self.bar = None;
                self.query = query.trim().to_string();
                true
            }
            InputEvent::Ignored if key.code == KeyCode::Esc => {
                self.bar = None;
                false
            }
            _ => false,
        }
    }

    /// The bar's text as the status line shows it, while it is up.
    pub fn bar_text(&self) -> Option<String> {
        self.bar.as_ref().map(|f| format!("/{}", f.value))
    }

    /// Mark the last search's matches inside the text.
    pub fn highlighted(&self, lines: Vec<Line>) -> Vec<Line> {
        if self.query.is_empty() {
            return lines;
        }
        lines
            .into_iter()
            .map(|l| {
                if l.is_book {
                    highlight(&l, &self.query)
                } else {
                    l
                }
            })
            .collect()
    }

    /// Search `book` from `start`: the paragraph to show (none if the query
    /// is missing), and the message to flash.
    pub fn find(
        &self,
        book: &Book,
        start: Pos,
        backward: bool,
        wrapped: bool,
    ) -> (Option<Pos>, Option<&'static str>) {
        match book.find(&self.query, start, backward) {
            None => (None, Some(NOT_FOUND_MSG)),
            Some((pos, wrapped_now)) => {
                let msg = (wrapped || wrapped_now).then(|| wrap_msg(backward));
                (Some(pos), msg)
            }
        }
    }
}

/// Where `f` / `F` searches from: the paragraph after (or before) `pos`, or
/// the other end of the text, and whether it wrapped around to get there.
pub fn next_start(book: &Book, pos: Pos, backward: bool) -> Option<(Pos, bool)> {
    let start = if backward {
        book.prev_pos(pos)
    } else {
        book.next_pos(pos)
    };
    let wrapped = start.is_none();
    let start = start.or_else(|| {
        let chapters = book.chapter_list();
        if backward {
            let (last, _) = *chapters.last()?;
            Some((last, book.chapters[last].paragraphs.len() - 1))
        } else {
            chapters.first().map(|(c, _)| (*c, 0))
        }
    });
    start.map(|s| (s, wrapped))
}
