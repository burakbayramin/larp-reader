//! Rendered lines and helpers shared by the masks (`masks/base.py`).
//!
//! A style is a space-separated string as in the Python version: colours
//! (`#d4d4d4`), `on #1f3d27` backgrounds, `bold`, `reverse`, and the internal
//! marker `book` on segments that carry the text. The UI turns it into
//! terminal styles; keeping it a string keeps masks and tests simple.

use unicode_width::UnicodeWidthChar;

use crate::book::Pos;
use crate::errors::FccError;
use crate::text::wrap;

/// (text, style).
pub type Segment = (String, String);

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

pub fn seg(text: impl Into<String>, style: impl Into<String>) -> Segment {
    (text.into(), style.into())
}

/// A non-text line of one styled piece.
pub fn plain_line(text: impl Into<String>, style: &str) -> Line {
    Line {
        segments: vec![seg(text, style)],
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
pub fn chrome(width: usize, segments: &[(&str, &str)]) -> Line {
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
            let style = l
                .segments
                .first()
                .map(|(_, s)| s.clone())
                .unwrap_or_default();
            chrome_owned(width, vec![(l.plain(), style)])
        })
        .collect()
}

pub fn error_lines(err: &FccError, width: usize) -> Vec<Line> {
    let title = format!(": {}", err.title());
    let mut lines = vec![chrome(
        width,
        &[("error", "bold #f85149"), (&title, "bold")],
    )];
    lines.extend(
        wrap(&err.to_string(), width)
            .into_iter()
            .map(|t| plain_line(t, "#f85149")),
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
        let line = chrome(5, &[("ab", "x"), ("cdef", "y")]);
        assert_eq!(line.segments, vec![seg("ab", "x"), seg("cde", "y")]);
        assert_eq!(chrome(3, &[("漢字", "x")]).plain(), "漢"); // a wide char that does not fit is dropped
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
