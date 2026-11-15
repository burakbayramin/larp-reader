//! Rendered lines and helpers shared by the masks (`masks/base.py`).
//!
//! Each segment has a `Look`: colour roles, modifiers, and the `book` flag on
//! segments that carry the text. The UI resolves it against the theme.

use unicode_width::UnicodeWidthChar;

use super::look::Look;
use super::palette::{BLUE, CYAN, DIM, FG, GREEN, GUTTER, RED, YELLOW};
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
    for (text, look) in segments {
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
        out.push((kept, look));
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

/// Crop single-look chrome lines to `width` cells.
pub fn fit_lines(lines: Vec<Line>, width: usize) -> Vec<Line> {
    lines
        .into_iter()
        .map(|l| {
            let look = l.segments.first().map(|(_, s)| *s).unwrap_or_default();
            chrome_owned(width, vec![(l.plain(), look)])
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

/// A status label in colour: the spinner and the "Verb…" in yellow, a
/// `[===>  ]` bar and percentages in green, counts like `142/380` bright,
/// times in blue, token counts in cyan, brackets and separators dim; the rest
/// in the status grey. The text stays exactly the label.
pub fn colour_label(label: &str) -> Vec<Segment> {
    fn push_into(out: &mut Vec<Segment>, text: &str, look: Look) {
        if text.is_empty() {
            return;
        }
        match out.last_mut() {
            Some((t, l)) if *l == look => t.push_str(text),
            _ => out.push((text.to_string(), look)),
        }
    }
    let mut out: Vec<Segment> = vec![];
    let mut rest = label;
    let mut prev_word = "";
    let mut first_word = true;
    while !rest.is_empty() {
        // a progress bar: [===>      ]
        if rest.starts_with('[') && rest.contains(']') {
            let end = rest.find(']').unwrap();
            let inner = &rest[1..end];
            if !inner.is_empty() && inner.chars().all(|c| matches!(c, '=' | '>' | ' ')) {
                push_into(&mut out, "[", DIM);
                for c in inner.chars() {
                    let mut b = [0; 4];
                    push_into(
                        &mut out,
                        c.encode_utf8(&mut b),
                        if c == ' ' { DIM } else { GREEN },
                    );
                }
                push_into(&mut out, "]", DIM);
                rest = &rest[end + 1..];
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        if c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | ',' | '·') {
            let mut b = [0; 4];
            push_into(&mut out, c.encode_utf8(&mut b), DIM);
            rest = &rest[c.len_utf8()..];
            continue;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | ',' | '·'))
            .unwrap_or(rest.len());
        let word = &rest[..end];
        rest = &rest[end..];
        let digits = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_digit() || c == '.');
        let look = if word.ends_with('…') {
            YELLOW.bold()
        } else if first_word && word.chars().count() == 1 {
            YELLOW // the spinner
        } else if word.strip_suffix('%').is_some_and(digits) {
            GREEN
        } else if word
            .split_once('/')
            .is_some_and(|(a, b)| digits(a) && digits(b))
        {
            FG.bold()
        } else if word == "↓" || prev_word == "↓" || word == "tokens" {
            CYAN
        } else if word.strip_suffix(['s', 'm', 'h']).is_some_and(digits) {
            BLUE
        } else if word.contains('/') || word.contains('.') && !digits(word) {
            CYAN // a path
        } else {
            GUTTER
        };
        push_into(&mut out, word, look);
        prev_word = word;
        first_word = false;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_label_keeps_the_text_and_colours_its_parts() {
        let labels = [
            "✻ Pondering… (12s · 3/10 · ↓ 1.2k tokens)",
            "Tailing [===>      ] 142/380",
            " src/indexer.rs  142/380  37%",
            "[ 37%] collected 380 items",
            "Resolving deltas:  37% (142/380)",
            "",
        ];
        for l in labels {
            let segs = colour_label(l);
            assert_eq!(segs.iter().map(|(t, _)| t.as_str()).collect::<String>(), l);
        }
        let look_of = |label: &str, part: &str| {
            colour_label(label)
                .into_iter()
                .find(|(t, _)| t.contains(part))
                .map(|(_, l)| l)
                .unwrap()
        };
        let agent = "✻ Pondering… (12s · 3/10 · ↓ 1.2k tokens)";
        assert_eq!(look_of(agent, "✻"), YELLOW);
        assert_eq!(look_of(agent, "Pondering…"), YELLOW.bold());
        assert_eq!(look_of(agent, "12s"), BLUE);
        assert_eq!(look_of(agent, "3/10"), FG.bold());
        assert_eq!(look_of(agent, "1.2k"), CYAN);
        assert_eq!(look_of("Tailing [===>      ] 142/380", "==="), GREEN);
        assert_eq!(look_of(" src/indexer.rs  142/380  37%", "37%"), GREEN);
        assert_eq!(
            look_of(" src/indexer.rs  142/380  37%", "src/indexer.rs"),
            CYAN
        );
    }

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
