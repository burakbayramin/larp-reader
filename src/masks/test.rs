//! A pytest run (`masks/test.py`): every paragraph is an assertion error, filler is passing tests.

use super::base::{Line, chrome, seg};
use super::filler::{Slot, test_ids_deck};
use super::palette::{DIM, FG, GREEN, RED};
use super::{Mask, Rng, randint, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use unicode_width::UnicodeWidthStr;

/// Python's `str.center(width, fill)`: the odd pad goes where CPython puts it.
fn center(text: &str, width: usize, fill: char) -> String {
    let len = text.width();
    if width <= len {
        return text.to_string();
    }
    let marg = width - len;
    let left = marg / 2 + (marg & width & 1);
    let f = fill.to_string();
    format!("{}{text}{}", f.repeat(left), f.repeat(marg - left))
}

#[derive(Default)]
pub struct TestMask {
    slot: Slot,
}

impl Mask for TestMask {
    fn name(&self) -> &'static str {
        "test"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        // pytest centres its banner on the terminal width
        let bold = FG.bold();
        vec![
            chrome(
                width,
                &[(&center(" test session starts ", width, '='), bold)],
            ),
            chrome(
                width,
                &[(
                    "platform linux -- Python 3.12.1, pytest-8.1.1, pluggy-1.4.0",
                    FG,
                )],
            ),
            chrome(width, &[("rootdir: /home/dev/project", FG)]),
            chrome(width, &[("collected 380 items", bold)]),
        ]
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let name = format!("{} ", test_ids_deck().get(self.slot.take()));
        let pct = format!(" [{:3}%]", randint(rng, 1, 100));
        chrome(width, &[(&name, FG), ("PASSED", GREEN), (&pct, DIM)])
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(4).max(1)) + 5
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.slot.set(line_no_start);
        // the failing test is its own entry of the pool, between the two passing ones
        let id = test_ids_deck().get(line_no_start + 1);
        let (fname, case) = id
            .trim_start_matches("tests/test_")
            .split_once(".py::test_")
            .expect("pool ids have this shape");
        let fail = RED.bold();
        let mut out = vec![
            self.noise(rng, width),
            chrome(width, &[(&format!("_____ test_{case} _____"), fail)]),
            chrome(width, &[("E   ", fail), ("AssertionError:", fail)]),
        ];
        for t in ragged(&para.text, width.saturating_sub(4).max(1)) {
            out.push(Line::book(vec![seg("E   ", fail), seg(t, FG.book())], pos));
        }
        let at = format!(":{line_no_start}: AssertionError");
        out.push(chrome(
            width,
            &[(&format!("tests/test_{fname}.py"), fail), (&at, FG)],
        ));
        self.slot.set(line_no_start + 2);
        out.push(self.noise(rng, width));
        out
    }

    /// The run reaching another test module: its progress line of dots.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let name = format!("tests/test_{}.py ", section::name(chapter, 0));
        let mut dots = ".".repeat(4 + chapter * 7 % 17);
        dots.push('F');
        let pct = format!(" [{:3}%]", 3 + chapter * 29 % 95);
        vec![chrome(width, &[(&name, FG), (&dots, GREEN), (&pct, DIM)])]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        let pct = (done * 100).checked_div(total).unwrap_or(0).min(100);
        format!("[{pct:3}%] collected {total} items")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Oda dar ve basıktı, tavanı alçaktı. ".repeat(6))
    }

    // -- test_test.py --

    #[test]
    fn header_is_pytest_session_banner() {
        let text: Vec<String> = TestMask::default()
            .header(&mut Rng::seed_from_u64(1), 80)
            .iter()
            .map(Line::plain)
            .collect();
        assert!(text[0].contains("test session starts"));
        assert!(
            text.iter()
                .any(|l| l.starts_with("collected ") && l.ends_with(" items"))
        );
    }

    #[test]
    fn banner_is_centred_to_the_width_like_pytest() {
        for width in [40, 60, 80, 41] {
            let banner = TestMask::default().header(&mut Rng::seed_from_u64(1), width)[0].plain();
            assert_eq!(banner.len(), width);
            let (left, right) = banner.split_once(" test session starts ").unwrap();
            assert!(left.chars().all(|c| c == '=') && right.chars().all(|c| c == '='));
            assert!(left.len().abs_diff(right.len()) <= 1);
        }
    }

    #[test]
    fn book_lines_are_assertion_error_message_lines() {
        let lines = TestMask::default().render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 1);
        let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(book.len() > 1);
        assert!(book.iter().all(|l| l.plain().starts_with("E   ")));
        assert!(
            book.iter()
                .all(|l| l.segments.last().unwrap().1 == FG.book())
        );
        let first = lines.iter().position(|l| l.is_book).unwrap();
        assert!(
            lines[..first]
                .iter()
                .any(|l| l.plain().starts_with("E   AssertionError"))
        );
    }

    #[test]
    fn noise_is_passed_test_line_with_percentage() {
        let mut r = Rng::seed_from_u64(5);
        for _ in 0..50 {
            let n = TestMask::default().noise(&mut r, 80);
            let p = n.plain();
            assert!(
                p.starts_with("tests/test_") && p.contains(".py::test_") && p.contains(" PASSED ["),
                "{p}"
            );
            assert!(p.ends_with("%]") && !n.is_book);
        }
    }

    #[test]
    fn progress_label() {
        assert_eq!(
            TestMask::default().progress_label(142, 380, 50),
            "[ 37%] collected 380 items"
        );
    }
}
