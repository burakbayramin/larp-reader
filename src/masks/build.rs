//! A cargo build (`masks/build.py`): every paragraph is a compiler warning's note, filler is crates compiling.

use super::base::{Line, chrome, progress_bar_label, seg};
use super::palette::{BLUE, FG, GREEN, YELLOW};
use super::{Mask, Rng, pick, randint};
use crate::book::{Paragraph, Pos};
use crate::text::ragged;
use unicode_width::UnicodeWidthStr;

const CRATES: &[&str] = &[
    "serde",
    "tokio",
    "regex",
    "anyhow",
    "hyper",
    "clap",
    "memchr",
    "once_cell",
    "bytes",
    "itoa",
];
const FILES: &[&str] = &["parser", "index", "render", "config", "cache"];
const VARS: &[&str] = &["chunk", "offset", "buf", "state", "count"];
const GUTTER: &str = "   |  ";

#[derive(Default)]
pub struct BuildMask;

impl Mask for BuildMask {
    fn name(&self) -> &'static str {
        "build"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![chrome(
            width,
            &[
                ("    Updating", &format!("bold {GREEN}")),
                (" crates.io index", FG),
            ],
        )]
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let ver = format!(
            "{}.{}.{}",
            randint(rng, 0, 2),
            randint(rng, 0, 20),
            randint(rng, 0, 9)
        );
        let name = format!(" {} v{ver}", pick(rng, CRATES));
        chrome(
            width,
            &[("   Compiling", &format!("bold {GREEN}")), (&name, FG)],
        )
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged(&para.text, width.saturating_sub(GUTTER.width()).max(1)).len() + 6
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        let rail = format!("bold {BLUE}");
        let file = pick(rng, FILES);
        let col = randint(rng, 5, 20);
        let var = pick(rng, VARS);
        let mut out = vec![
            self.noise(rng, width),
            chrome(
                width,
                &[
                    ("warning", &format!("bold {YELLOW}")),
                    (
                        &format!(": unused variable: `{var}`"),
                        &format!("bold {FG}"),
                    ),
                ],
            ),
            chrome(
                width,
                &[
                    ("  --> ", &rail),
                    (&format!("src/{file}.rs:{line_no_start}:{col}"), FG),
                ],
            ),
            chrome(width, &[("   |", &rail)]),
        ];
        for t in ragged(&para.text, width.saturating_sub(GUTTER.width()).max(1)) {
            out.push(Line::book(
                vec![seg(GUTTER, rail.clone()), seg(t, format!("book {FG}"))],
                pos,
            ));
        }
        out.push(chrome(width, &[("   |", &rail)]));
        out.push(self.noise(rng, width));
        out
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Building")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Oda dar ve basıktı, tavanı alçaktı. ".repeat(6))
    }

    // -- test_build.py --

    #[test]
    fn noise_is_compiling_line_with_semver() {
        let mut r = Rng::seed_from_u64(6);
        for _ in 0..50 {
            let n = BuildMask.noise(&mut r, 80);
            let p = n.plain();
            let (name, ver) = p
                .trim_start_matches("   Compiling ")
                .split_once(" v")
                .unwrap();
            assert!(
                p.starts_with("   Compiling ")
                    && name.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{p}"
            );
            assert_eq!(
                ver.split('.').filter(|x| x.parse::<u32>().is_ok()).count(),
                3,
                "{p}"
            );
            assert!(!n.is_book);
        }
    }

    #[test]
    fn book_lines_are_warning_note_lines() {
        let lines = BuildMask.render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 41);
        let plain: Vec<String> = lines.iter().map(Line::plain).collect();
        assert!(plain.iter().any(|p| p.starts_with("warning:")));
        assert!(
            plain
                .iter()
                .any(|p| p.starts_with("  --> src/") && p.contains(".rs:41:"))
        );
        let book: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(book.len() > 1 && book.iter().all(|l| l.plain().starts_with("   |  ")));
        assert!(
            book.iter()
                .all(|l| l.segments.last().unwrap().1 == format!("book {FG}"))
        );
    }

    #[test]
    fn location_changes_with_line_no_start() {
        let arrow = |s| -> Vec<String> {
            BuildMask
                .render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), s)
                .iter()
                .map(Line::plain)
                .filter(|p| p.contains("-->"))
                .collect()
        };
        assert_ne!(arrow(1), arrow(400));
    }

    #[test]
    fn progress_label() {
        assert_eq!(
            BuildMask.progress_label(142, 380, 50),
            "Building [===>      ] 142/380"
        );
    }
}
