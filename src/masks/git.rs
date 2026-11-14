//! `git log --stat` (`masks/git.py`): every paragraph is a commit message.

use rand::Rng as _;

use super::base::{Line, chrome, seg};
use super::filler::{Slot, git_paths_deck};
use super::palette::{DIM, FG, GREEN, RED, YELLOW};
use super::time::{EPOCH_DAY, civil, month, weekday};
use super::{Mask, Rng, pick, randint, section};
use crate::book::{Paragraph, Pos};
use crate::text::{pad_cells, ragged, ragged_count};

/// 2026-11-11 18:00, in minutes since 1970: history counts back from here.
const EPOCH_MIN: i64 = EPOCH_DAY * 1440 + 18 * 60;
const AUTHORS: &[(&str, &str)] = &[
    ("Deniz Aksoy", "deniz.aksoy@example.com"),
    ("Mert Kaya", "mert@example.com"),
    ("Elif Demir", "elif.demir@example.com"),
    ("Alex Novak", "anovak@example.com"),
    ("Sam Patel", "sam.patel@example.com"),
];
const INDENT: usize = 4;
/// commit, Author, Date, blank.
const HEADER_LINES: usize = 4;
const STAT_WIDTH: usize = 34;

fn indent(width: usize) -> usize {
    INDENT.min(width / 8)
}

/// `Wed Nov 11 17:44:00 2026 +0300`.
fn git_date(minutes: i64) -> String {
    let days = minutes.div_euclid(1440);
    let (y, m, d) = civil(days);
    let min = minutes.rem_euclid(1440);
    format!(
        "{} {} {d:02} {:02}:{:02}:00 {y} +0300",
        weekday(days),
        month(m),
        min / 60,
        min % 60
    )
}

/// One `git log --stat` file line: ` path | 12 +++++---`.
fn stat(rng: &mut Rng, width: usize, slot: usize) -> Line {
    let path = git_paths_deck().get(slot);
    let mut adds = randint(rng, 0, 14);
    let dels = randint(rng, 0, 9);
    if adds + dels == 0 {
        adds = 1;
    }
    let name = format!(" {path}");
    let name = if width >= 60 {
        pad_cells(&name, STAT_WIDTH)
    } else {
        name
    };
    chrome(
        width,
        &[
            (&format!("{name} | {:>3} ", adds + dels), FG),
            (&"+".repeat(adds), GREEN),
            (&"-".repeat(dels), RED),
        ],
    )
}

#[derive(Default)]
pub struct GitMask {
    slot: Slot,
}

impl Mask for GitMask {
    fn name(&self) -> &'static str {
        "git"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![chrome(width, &[("git log --stat --first-parent", DIM)])]
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        stat(rng, width, self.slot.take())
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        HEADER_LINES + ragged_count(&para.text, width.saturating_sub(indent(width)).max(1)) + 1
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
        let sha: String = (0..5)
            .map(|_| format!("{:08x}", rng.r#gen::<u32>()))
            .collect();
        let (author, mail) = pick(rng, AUTHORS);
        // newest first, like the real thing: later text is older history
        let when = EPOCH_MIN - (line_no_start as i64 * 7 + randint(rng, 0, 6) as i64);
        let mut out = vec![
            chrome(width, &[(&format!("commit {sha}"), YELLOW)]),
            chrome(width, &[(&format!("Author: {author} <{mail}>"), FG)]),
            chrome(width, &[(&format!("Date:   {}", git_date(when)), FG)]),
            chrome(width, &[("", FG)]),
        ];
        let ind = indent(width);
        for text in ragged(&para.text, width.saturating_sub(ind).max(1)) {
            out.push(Line::book(
                vec![seg(" ".repeat(ind), ""), seg(text, format!("book {FG}"))],
                pos,
            ));
        }
        out.push(chrome(width, &[("", FG)]));
        out
    }

    /// A merge commit: the branch whose commits follow.
    fn transition(&self, chapter: usize, width: usize, line_no_start: usize) -> Vec<Line> {
        let (author, mail) = section::choose(AUTHORS, chapter);
        // newer than the commits below it, older than those above
        let when = EPOCH_MIN - line_no_start as i64 * 7;
        let kind = section::choose(&["feature", "fix", "chore", "feature", "refactor"], chapter);
        let branch = format!(
            "{kind}/{}-{}",
            section::name(chapter, 0),
            section::name(chapter, 1)
        );
        let ind = " ".repeat(indent(width));
        vec![
            chrome(
                width,
                &[(
                    &format!("commit {}", section::hex(chapter, "merge", 40)),
                    YELLOW,
                )],
            ),
            chrome(
                width,
                &[(
                    &format!(
                        "Merge: {} {}",
                        section::hex(chapter, "parent-1", 7),
                        section::hex(chapter, "parent-2", 7)
                    ),
                    FG,
                )],
            ),
            chrome(width, &[(&format!("Author: {author} <{mail}>"), FG)]),
            chrome(width, &[(&format!("Date:   {}", git_date(when)), FG)]),
            chrome(width, &[("", FG)]),
            chrome(width, &[(&format!("{ind}Merge branch '{branch}'"), FG)]),
            chrome(width, &[("", FG)]),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        let pct = (done * 100).checked_div(total).unwrap_or(0).min(100);
        format!("Resolving deltas: {pct:3}% ({done}/{total})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Sabah erkenden uyandı ve pencereyi açtı. ".repeat(4))
    }

    // -- test_git_docker.py (git) --

    #[test]
    fn git_paragraph_is_an_indented_commit_message() {
        let lines = GitMask::default().render(&para(), (0, 0), 80, &mut Rng::seed_from_u64(1), 10);
        let first = lines[0].plain();
        assert!(first.starts_with("commit ") && first.len() == 47, "{first}");
        assert!(lines[1].plain().starts_with("Author: "));
        assert!(lines[2].plain().starts_with("Date:   "));
        let body: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(!body.is_empty() && body.iter().all(|l| l.plain().starts_with("    ")));
    }

    #[test]
    fn git_history_goes_back_in_time() {
        let first = GitMask::default().render(&para(), (0, 0), 80, &mut Rng::seed_from_u64(1), 1)
            [2]
        .plain();
        let later = GitMask::default().render(&para(), (0, 1), 80, &mut Rng::seed_from_u64(1), 500)
            [2]
        .plain();
        assert_ne!(first, later);
        assert_eq!(git_date(EPOCH_MIN - 16), "Wed Nov 11 17:44:00 2026 +0300");
    }

    #[test]
    fn git_filler_is_a_stat_line() {
        let line = GitMask::default()
            .noise(&mut Rng::seed_from_u64(2), 80)
            .plain();
        assert!(
            line.contains(" | ") && line.trim_end().ends_with(['+', '-']),
            "{line}"
        );
    }
}
