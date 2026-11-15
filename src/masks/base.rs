//! Rendered lines and helpers shared by the masks (`masks/base.py`).
//!
//! Each segment has a `Look`: colour roles, modifiers, and the `book` flag on
//! segments that carry the text. The UI resolves it against the theme.

use unicode_width::UnicodeWidthChar;

use super::look::Look;
use super::palette::RED;
use crate::book::Pos;
use crate::errors::FccError;
use crate::text::wrap;

/// (text, look).
pub type Segment = (String, Look);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub segments: Vec<Segment>,
    /// Carries text of the book (counts for reading, progress and search).
    pub is_book: bool,
    pub pos: Option<Pos>,
}

impl Line {
    pub fn plain(&self) -> String {
        self.segments.iter().map(|(t, _)| t.as_str()).collect()
    }

    /// A text line: `segments` as given, tagged with the paragraph it shows.
    pub fn book(segments: Vec<Segment>, pos: Pos) -> Line {
        Line {
            segments,
            is_book: true,
            pos: Some(pos),
        }
    }
}

pub fn seg(text: impl Into<String>, look: Look) -> Segment {
    (text.into(), look)
}

/// A non-text line of one styled piece.
pub fn plain_line(text: impl Into<String>, look: Look) -> Line {
    Line {
        segments: vec![seg(text, look)],
        is_book: false,
        pos: None,
    }
}

/// The leading `width` cells of a segmented line.
pub fn crop(segments: Vec<Segment>, width: usize) -> Vec<Segment> {
    let mut out = vec![];
    let mut left = width;
    for (text, style) in segments {
        if left == 0 {
            break;
        }
        let mut kept = String::new();
        for c in text.chars() {
            let w = c.width().unwrap_or(0);
            if w > left {
                left = 0;
                break;
            }
            kept.push(c);
            left -= w;
        }
        out.push((kept, style));
    }
    out
}

/// A non-text line of several styled pieces, cropped to `width` cells.
pub fn chrome(width: usize, segments: &[(&str, Look)]) -> Line {
    let owned = segments.iter().map(|(t, s)| seg(*t, *s)).collect();
    Line {
        segments: crop(owned, width.max(1)),
        is_book: false,
        pos: None,
    }
}

/// Like `chrome`, for pieces built at run time.
pub fn chrome_owned(width: usize, segments: Vec<Segment>) -> Line {
    Line {
        segments: crop(segments, width.max(1)),
        is_book: false,
        pos: None,
    }
}

/// Crop single-style chrome lines to `width` cells.
pub fn fit_lines(lines: Vec<Line>, width: usize) -> Vec<Line> {
    lines
        .into_iter()
        .map(|l| {
            let style = l.segments.first().map(|(_, s)| *s).unwrap_or_default();
            chrome_owned(width, vec![(l.plain(), style)])
        })
        .collect()
}

pub fn error_lines(err: &FccError, width: usize) -> Vec<Line> {
    let title = format!(": {}", err.title());
    let mut lines = vec![chrome(
        width,
        &[("error", RED.bold()), (&title, Look::PLAIN.bold())],
    )];
    lines.extend(
        wrap(&err.to_string(), width)
            .into_iter()
            .map(|t| plain_line(t, RED)),
    );
    lines
}

/// `Indexing [=====>    ] 142/380`.
pub fn progress_bar_label(done: usize, total: usize, cells: usize, label: &str) -> String {
    let filled = (done * cells).checked_div(total).unwrap_or(0).min(cells);
    let bar = if filled >= cells {
        "=".repeat(cells)
    } else {
        format!("{}>{}", "=".repeat(filled), " ".repeat(cells - filled - 1))
    };
    format!("{label} [{bar}] {done}/{total}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_counts_cells_and_keeps_styles() {
        let (x, y) = (Look::PLAIN, Look::PLAIN.bold());
        let line = chrome(5, &[("ab", x), ("cdef", y)]);
        assert_eq!(line.segments, vec![seg("ab", x), seg("cde", y)]);
        assert_eq!(chrome(3, &[("漢字", x)]).plain(), "漢"); // a wide char that does not fit is dropped
    }

    #[test]
    fn error_lines_show_title_and_reason() {
        let lines = error_lines(&FccError::NoTextLayer("Metin katmanı yok".into()), 60);
        assert_eq!(lines[0].plain(), "error: no text layer found");
        assert_eq!(lines[1].plain(), "Metin katmanı yok");
    }

    #[test]
    fn progress_bar() {
        assert_eq!(
            progress_bar_label(0, 10, 10, "Indexing"),
            "Indexing [>         ] 0/10"
        );
        assert_eq!(
            progress_bar_label(5, 10, 10, "Indexing"),
            "Indexing [=====>    ] 5/10"
        );
        assert_eq!(
            progress_bar_label(10, 10, 10, "Indexing"),
            "Indexing [==========] 10/10"
        );
        assert_eq!(progress_bar_label(3, 0, 4, "X"), "X [>   ] 3/0");
    }
}
