//! A `jupyter console` session: every paragraph is the `Out[n]` of a cell that
//! looks a value up, filler is other cells being typed, and a new chapter runs
//! another notebook script.

use std::sync::OnceLock;

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::codegen::Deck;
use super::filler::{Slot, expand, seq, words};
use super::palette::{BLUE, CYAN, DIM, FG, GREEN, RED, highlight};
use super::{Mask, Rng, randint, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};

/// Output lines are indented this much under their `Out[n]:`.
const INDENT: usize = 4;
/// Lines of a cell around its output lines.
const FRAME: usize = 3;
const CONTINUE: &str = "   ...: ";
/// (frame, column), chosen by line number.
const SOURCES: &[(&str, &str)] = &[
    ("notes", "body"),
    ("df", "text"),
    ("corpus", "passage"),
    ("samples", "content"),
    ("frame", "summary"),
];

/// The number of the cell drawn at line `line_no`: counts up through the session.
/// A cell takes two lines or more (a text cell four), so filler cells numbered on
/// from a text cell stay below the next text cell's number.
fn cell_no(line_no: usize) -> usize {
    1 + line_no / 2
}

fn in_prompt(n: usize) -> Segment {
    seg(format!("In [{n}]: "), GREEN)
}

/// A line of Python after `lead`, syntax coloured.
fn code_line(lead: Segment, code: &str, width: usize) -> Line {
    let mut segs = vec![lead];
    segs.extend(
        highlight("python", code)
            .into_iter()
            .next()
            .unwrap_or_default(),
    );
    chrome_owned(width, segs)
}

// -- filler ----------------------------------------------------------------------

fn vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "df",
            words(&[
                "df", "events", "orders", "users", "sessions", "metrics", "logs",
            ]),
        ),
        (
            "c",
            words(&[
                "status",
                "latency_ms",
                "region",
                "created_at",
                "amount",
                "user_id",
                "kind",
                "score",
                "retries",
                "duration",
            ]),
        ),
        (
            "c2",
            words(&[
                "amount",
                "latency_ms",
                "score",
                "duration",
                "retries",
                "size",
            ]),
        ),
        (
            "f",
            words(&[
                "events_2026_11",
                "orders_q3",
                "sessions_raw",
                "metrics_daily",
                "users_snapshot",
                "logs_sample",
            ]),
        ),
        ("n", seq(10, 15, 20)),
        ("k", seq(2, 1, 12)),
        ("q", words(&["0.5", "0.9", "0.95", "0.99"])),
    ]
}

const CELLS: &[&str] = &[
    "{df} = pd.read_parquet(\"data/{f}.parquet\")",
    "{df}.groupby(\"{c}\")[\"{c2}\"].agg([\"mean\", \"count\"])",
    "{df}[{df}.{c2} > {n}].shape",
    "{df}.{c}.value_counts().head({k})",
    "{df}.{c2}.quantile({q})",
    "{df}.{c2}.rolling({k}).mean().plot(figsize=(12, 4))",
    "%timeit {df}.{c2}.sum()",
    "{df}.sort_values(\"{c2}\", ascending=False).head({k})",
    "{df}[\"{c}\"].isna().mean()",
    "{df}.pivot_table(index=\"{c}\", values=\"{c2}\", aggfunc=\"median\")",
];
const BODIES: &[&str] = &[
    "    {df} = {df}[{df}.{c2}.notna()]",
    "    {df}[\"{c2}\"] = {df}.{c2}.clip(upper={n})",
    "    print({df}.{c}.nunique(), len({df}))",
    "    {df} = {df}.merge(lookup, on=\"{c}\", how=\"left\")",
    "    {df}[\"{c2}_z\"] = ({df}.{c2} - {df}.{c2}.mean()) / {df}.{c2}.std()",
    "    {df}.to_parquet(\"out/{f}_{k}.parquet\")",
];

fn cells_deck() -> &'static Deck {
    static D: OnceLock<Deck> = OnceLock::new();
    D.get_or_init(|| Deck::new(expand(CELLS, &vocab(), 120), "jupyter-cells"))
}

fn bodies_deck() -> &'static Deck {
    static D: OnceLock<Deck> = OnceLock::new();
    D.get_or_init(|| Deck::new(expand(BODIES, &vocab(), 120), "jupyter-bodies"))
}

#[derive(Default)]
pub struct JupyterMask {
    slot: Slot,
    /// Number of the next filler cell.
    cell: usize,
}

impl JupyterMask {
    /// A filler cell of `size` lines: the input line, continuation lines, a blank.
    fn cell_lines(&mut self, size: usize, width: usize) -> Vec<Line> {
        self.cell += 1;
        let mut out = vec![code_line(
            in_prompt(self.cell),
            cells_deck().get(self.slot.take()),
            width,
        )];
        if size >= 2 {
            for _ in 0..size - 2 {
                out.push(code_line(
                    seg(CONTINUE, GREEN),
                    bodies_deck().get(self.slot.take()),
                    width,
                ));
            }
            out.push(chrome(width, &[]));
        }
        out
    }
}

impl Mask for JupyterMask {
    fn name(&self) -> &'static str {
        "jupyter"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![
            chrome(width, &[("Jupyter console 6.6.3", FG)]),
            chrome(width, &[]),
            chrome(
                width,
                &[(
                    "Python 3.12.7 | packaged by conda-forge | (main, Oct  4 2024, 16:05:46) [GCC 13.3.0]",
                    FG,
                )],
            ),
            chrome(
                width,
                &[(
                    "Type 'copyright', 'credits' or 'license' for more information",
                    FG,
                )],
            ),
            chrome(
                width,
                &[(
                    "IPython 8.29.0 -- An enhanced Interactive Python. Type '?' for help.",
                    FG,
                )],
            ),
            chrome(width, &[]),
        ]
    }

    fn stream(&mut self) {
        self.slot.stream();
    }

    /// The hidden screen: one-line cells.
    fn noise(&mut self, _rng: &mut Rng, width: usize) -> Line {
        self.cell_lines(1, width).remove(0)
    }

    /// Whole cells of two to four lines; a single line is a blank one.
    fn noise_block(&mut self, count: usize, rng: &mut Rng, width: usize) -> Option<Vec<Line>> {
        if count == 1 {
            return Some(vec![chrome(width, &[])]);
        }
        let mut out = vec![];
        let mut left = count;
        while left > 0 {
            // never leave a single line over when more than one cell is coming
            let size = if left <= 4 {
                left
            } else {
                randint(rng, 2, 4).min(left - 2)
            };
            out.extend(self.cell_lines(size, width));
            left -= size;
        }
        Some(out)
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(INDENT).max(1)) + FRAME
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.slot.set(line_no_start);
        let n = cell_no(line_no_start);
        self.cell = n;
        let (frame, col) = SOURCES[line_no_start % SOURCES.len()];
        let row = 100 + line_no_start % 9_900;
        let code = format!("{frame}.loc[{row}, \"{col}\"]");
        let mut out = vec![
            code_line(in_prompt(n), &code, width),
            chrome(width, &[(&format!("Out[{n}]:"), RED)]),
        ];
        for t in ragged(&para.text, width.saturating_sub(INDENT).max(1)) {
            out.push(Line::book(
                vec![seg(" ".repeat(INDENT), FG), seg(t, FG.book())],
                pos,
            ));
        }
        out.push(chrome(width, &[]));
        out
    }

    /// Another notebook script run into the session.
    fn transition(&self, chapter: usize, width: usize, line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        let rows = 1_200 + chapter * 4_271 % 90_000;
        vec![
            code_line(
                in_prompt(cell_no(line_no_start)),
                &format!("%run -i scripts/{name}.py"),
                width,
            ),
            chrome(
                width,
                &[(&format!("loaded {name}: {rows} rows, 14 columns"), FG)],
            ),
            chrome(width, &[]),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Executing")
    }

    fn status_right(
        &self,
        _done: usize,
        _total: usize,
        _elapsed: std::time::Duration,
    ) -> Vec<Segment> {
        vec![
            seg("Python 3 (ipykernel)", CYAN),
            seg("  ·  ", DIM),
            seg("○ idle", GREEN),
            seg("  ·  ", DIM),
            seg("analysis.ipynb", BLUE),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Sabah erkenden uyandı ve pencereyi açtı. ".repeat(4))
    }

    #[test]
    fn header_is_the_console_banner() {
        let text: Vec<String> = JupyterMask::default()
            .header(&mut Rng::seed_from_u64(1), 100)
            .iter()
            .map(Line::plain)
            .collect();
        assert!(text[0].starts_with("Jupyter console"));
        assert!(text.iter().any(|l| l.starts_with("IPython ")));
    }

    #[test]
    fn a_paragraph_is_the_output_of_a_cell() {
        let lines =
            JupyterMask::default().render(&para(), (0, 0), 70, &mut Rng::seed_from_u64(1), 30);
        let text: Vec<String> = lines.iter().map(Line::plain).collect();
        assert!(text[0].starts_with("In [16]: ") && text[0].contains(".loc["));
        assert_eq!(text[1], "Out[16]:");
        let book: Vec<&String> = text[2..text.len() - 1].iter().collect();
        assert!(book.len() > 1 && book.iter().all(|l| l.starts_with("    ")));
        assert_eq!(text.last().unwrap(), "");
    }

    #[test]
    fn noise_blocks_are_whole_cells_of_the_exact_size() {
        for count in 0..=12 {
            let mut m = JupyterMask::default();
            let block = m
                .noise_block(count, &mut Rng::seed_from_u64(count as u64), 100)
                .unwrap();
            assert_eq!(block.len(), count);
            if count > 1 {
                assert!(block[0].plain().starts_with("In ["));
            }
            assert!(!block.iter().any(|l| l.is_book));
        }
    }

    #[test]
    fn cells_count_up() {
        let mut m = JupyterMask::default();
        let mut r = Rng::seed_from_u64(1);
        m.render(&para(), (0, 0), 80, &mut r, 30);
        assert!(m.noise(&mut r, 80).plain().starts_with("In [17]: "));
        assert!(m.noise(&mut r, 80).plain().starts_with("In [18]: "));
    }

    #[test]
    fn pools_are_large_and_clean() {
        for d in [cells_deck(), bodies_deck()] {
            assert!(d.len() >= 400, "{}", d.len());
            for i in 0..d.len() {
                let low = d.get(i).to_lowercase();
                assert!(!low.contains("book") && !low.contains("reader"), "{low}");
            }
        }
    }
}
