//! A cargo build (`masks/build.py`): every paragraph is a compiler warning's note, filler is crates compiling.

use super::base::{Line, chrome, progress_bar_label, seg};
use super::codegen::{MODULES, VARS};
use super::filler::{Slot, crates_deck};
use super::palette::{BLUE, FG, GREEN, YELLOW};
use super::{Mask, Rng, randint, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use unicode_width::UnicodeWidthStr;

const GUTTER: &str = "   |  ";

#[derive(Default)]
pub struct BuildMask {
    slot: Slot,
}

impl Mask for BuildMask {
    fn name(&self) -> &'static str {
        "build"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![chrome(
            width,
            &[("    Updating", GREEN.bold()), (" crates.io index", FG)],
        )]
    }

    fn noise(&mut self, _rng: &mut Rng, width: usize) -> Line {
        let name = format!(" {}", crates_deck().get(self.slot.take()));
        chrome(width, &[("   Compiling", GREEN.bold()), (&name, FG)])
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(GUTTER.width()).max(1)) + 6
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        let rail = BLUE.bold();
        self.slot.set(line_no_start);
        let file = MODULES[line_no_start * 7 % MODULES.len()];
        let col = randint(rng, 5, 20);
        let var = VARS[line_no_start * 5 % VARS.len()];
        let mut out = vec![
            self.noise(rng, width),
            chrome(
                width,
                &[
                    ("warning", YELLOW.bold()),
                    (&format!(": unused variable: `{var}`"), FG.bold()),
                ],
            ),
            chrome(
                width,
                &[
                    ("  --> ", rail),
                    (&format!("src/{file}.rs:{line_no_start}:{col}"), FG),
                ],
            ),
            chrome(width, &[("   |", rail)]),
        ];
        for t in ragged(&para.text, width.saturating_sub(GUTTER.width()).max(1)) {
            out.push(Line::book(vec![seg(GUTTER, rail), seg(t, FG.book())], pos));
        }
        out.push(chrome(width, &[("   |", rail)]));
        out.push(self.noise(rng, width));
        out
    }

    /// A crate boundary in the workspace: the last crate's warning tally, the next one compiling.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let prev = section::name(chapter.saturating_sub(1), 2);
        let next = section::name(chapter, 2);
        let n = 2 + chapter % 5;
        vec![
            chrome(
                width,
                &[
                    ("warning", YELLOW.bold()),
                    (
                        &format!(": `core-{prev}` (lib) generated {n} warnings"),
                        FG.bold(),
                    ),
                ],
            ),
            chrome(
                width,
                &[
                    ("   Compiling", GREEN.bold()),
                    (
                        &format!(
                            " core-{next} v{} (/home/dev/project/crates/{next})",
                            section::version(chapter)
                        ),
                        FG,
                    ),
                ],
            ),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Building")
    }

    fn status_right(
        &self,
        done: usize,
        total: usize,
        elapsed: std::time::Duration,
    ) -> Vec<super::base::Segment> {
        let _ = (total, elapsed);
        vec![
            super::base::seg("jobs: 8", super::palette::BLUE),
            super::base::seg("  ·  ", super::palette::DIM),
            super::base::seg(format!("warnings: {}", done / 37), super::palette::YELLOW),
            super::base::seg("  ·  ", super::palette::DIM),
            super::base::seg("release", super::palette::GREEN),
        ]
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
            let n = BuildMask::default().noise(&mut r, 80);
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
        let lines =
            BuildMask::default().render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 41);
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
                .all(|l| l.segments.last().unwrap().1 == FG.book())
        );
    }

    #[test]
    fn location_changes_with_line_no_start() {
        let arrow = |s| -> Vec<String> {
            BuildMask::default()
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
            BuildMask::default().progress_label(142, 380, 50),
            "Building [===>      ] 142/380"
        );
    }
}
