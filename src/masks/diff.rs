//! A diff (`masks/diff.py`): every paragraph is a hunk whose added lines are the text.

use super::base::{Line, chrome, chrome_owned, progress_bar_label, seg};
use super::palette::{CYAN, DIM, FG, GREEN, RED, highlight};
use super::{Mask, Rng, pick, random};
use crate::book::{Paragraph, Pos};
use crate::text::ragged;
use unicode_width::UnicodeWidthStr;

const CONTEXT: &[&str] = &[
    "    ctx = build_context(request)",
    "    if not payload:",
    "        return None",
    "    for item in queue:",
    "    return response",
    "    logger.debug(\"handled\")",
];
const REMOVED: &[&str] = &[
    "    result = old_handler(request)",
    "    cache.clear()",
    "    timeout = 30",
    "    raise NotImplementedError",
];

#[derive(Default)]
pub struct DiffMask;

impl Mask for DiffMask {
    fn name(&self) -> &'static str {
        "diff"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        // a shorter path in narrow panes, rather than a cut-off `diff --git` line
        let full = "diff --git a/src/handler.py b/src/handler.py";
        let path = if full.width() <= width {
            "src/handler.py"
        } else {
            "handler.py"
        };
        let head = format!("bold {FG}");
        vec![
            chrome(width, &[(&format!("diff --git a/{path} b/{path}"), &head)]),
            chrome(width, &[("index 3f2a1c4..9b7e0d2 100644", DIM)]),
            chrome(width, &[(&format!("--- a/{path}"), &head)]),
            chrome(width, &[(&format!("+++ b/{path}"), &head)]),
        ]
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        if random(rng) < 0.6 {
            let mut segs = vec![seg(" ", FG)];
            segs.extend(highlight("python", pick(rng, CONTEXT)).swap_remove(0));
            return chrome_owned(width, segs);
        }
        chrome(
            width,
            &[("-", RED), (&format!(" {}", pick(rng, REMOVED)), RED)],
        )
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged(&para.text, width.saturating_sub(2).max(1)).len() + 3
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        let texts = ragged(&para.text, width.saturating_sub(2).max(1));
        let start = 100 + line_no_start;
        let hunk = format!("@@ -{start},7 +{start},{} @@", 7 + texts.len());
        let mut out = vec![chrome(width, &[(&hunk, CYAN), (" def handler():", FG)])];
        out.push(self.noise(rng, width));
        for t in texts {
            out.push(Line::book(
                vec![seg("+ ", GREEN), seg(t, format!("book {GREEN}"))],
                pos,
            ));
        }
        out.push(self.noise(rng, width));
        out
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Diffing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Oda dar ve basıktı, tavanı alçaktı. ".repeat(6))
    }

    // -- test_diff.py --

    #[test]
    fn header_is_a_diff_preamble() {
        let lines: Vec<String> = DiffMask
            .header(&mut Rng::seed_from_u64(1), 80)
            .iter()
            .map(Line::plain)
            .collect();
        assert_eq!(lines[0], "diff --git a/src/handler.py b/src/handler.py");
        assert!(
            lines.iter().any(|l| l.starts_with("--- a/src/"))
                && lines.iter().any(|l| l.starts_with("+++ b/src/"))
        );
    }

    #[test]
    fn paragraph_starts_with_hunk_header_and_book_lines_are_additions() {
        let lines = DiffMask.render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 41);
        let first = lines[0].plain();
        assert!(
            first.starts_with("@@ -141,7 +141,") && first.ends_with(" @@ def handler():"),
            "{first}"
        );
        assert!(!lines[0].is_book);
        let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(book.len() > 1);
        assert!(book.iter().all(|l| l.plain().starts_with("+ ")));
        assert!(
            book.iter()
                .all(|l| l.segments.last().unwrap().1 == format!("book {GREEN}"))
        );
    }

    #[test]
    fn hunk_header_changes_with_line_no_start() {
        let a = DiffMask.render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 1)[0].plain();
        let b = DiffMask.render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 400)[0].plain();
        assert_ne!(a, b);
    }

    #[test]
    fn noise_is_context_or_removal_lines() {
        let mut r = Rng::seed_from_u64(4);
        let kinds: std::collections::HashSet<String> = (0..100)
            .map(|_| DiffMask.noise(&mut r, 70).plain().chars().take(2).collect())
            .collect();
        assert_eq!(kinds, ["  ".to_string(), "- ".to_string()].into());
        assert!((0..20).all(|_| !DiffMask.noise(&mut r, 70).is_book));
    }

    #[test]
    fn progress_label() {
        assert_eq!(
            DiffMask.progress_label(142, 380, 50),
            "Diffing [===>      ] 142/380"
        );
    }
}
