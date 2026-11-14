//! A coding-agent session (`masks/agent.py`): tool calls, coloured diffs, command output and narration.
//!
//! Most paragraphs appear as the added lines of an edit, the rest as the
//! agent's own remarks. Filler comes from the shuffled pools in `codegen`,
//! read by a "slot": the line number where a paragraph or filler unit starts.
//! Line numbers are unique along the text, so neighbours read different parts
//! of the pools, and redrawing a paragraph gives the same lines.

use std::collections::VecDeque;
use std::time::Duration;

use rand::Rng as _;

use super::base::{Line, Segment, chrome, chrome_owned, crop, seg};
use super::codegen::{commands_deck, files_deck, signatures_deck, statements_deck, tasks_deck};
use super::palette::{DIM, FG, GREEN, RED, highlight};
use super::{Mask, Rng, pick, randint, random};
use crate::book::{Paragraph, Pos};
use crate::text::ragged;
use unicode_width::UnicodeWidthStr;

pub const BULLET: &str = if cfg!(windows) { "●" } else { "⏺" };
/// First line of a tool result, and its following lines.
const ELBOW: &str = "  ⎿  ";
const CONT: &str = "     ";
pub const ADD_BG: &str = "on #1f3d27";
pub const DEL_BG: &str = "on #4b1f22";
const THINKING: &str = "✻ Thinking…";
/// "%7d " before the +/- sign.
const GUTTER: usize = 8;
/// Sign and a space.
const SIGN: usize = 2;
/// Statements sit inside a method.
const BODY_INDENT: usize = 8;
/// Statements one slot may use.
const PER_SLOT: usize = 8;
/// The hidden stream reads a part of the pools the text never reaches.
const STREAM_BASE: usize = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The text as added lines of an edit.
    Edit,
    /// The text as narration.
    Say,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub kind: Kind,
    /// Lines the edit takes out.
    pub removed: usize,
    /// Unchanged lines shown above the change.
    pub before: usize,
}

/// How a paragraph is laid out; from its text alone, so it can be counted without drawing.
pub fn layout(text: &str) -> Layout {
    let h = crc32fast::hash(text.as_bytes());
    let kind = if h % 10 < 3 { Kind::Say } else { Kind::Edit };
    Layout {
        kind,
        removed: (h >> 4) as usize % 4,
        before: 1 + (h >> 8) as usize % 2,
    }
}

fn code(line: &str) -> Vec<Segment> {
    if line.trim().is_empty() {
        vec![]
    } else {
        highlight("python", line).swap_remove(0)
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn indent(width: usize) -> usize {
    BODY_INDENT.min(width.saturating_sub(GUTTER + SIGN) / 10)
}

fn body_width(width: usize) -> usize {
    width.saturating_sub(GUTTER + SIGN + indent(width)).max(1)
}

/// Fill the line out to `width` with the background colour.
fn pad(mut segments: Vec<Segment>, width: usize, bg: &str) -> Vec<Segment> {
    let used: usize = segments.iter().map(|(t, _)| t.width()).sum();
    if used < width {
        segments.push(seg(" ".repeat(width - used), bg));
    }
    segments
}

fn stmt(slot: usize, j: usize) -> String {
    format!(
        "{}{}",
        " ".repeat(BODY_INDENT),
        statements_deck().get(slot * PER_SLOT + j)
    )
}

fn signature(slot: usize) -> String {
    format!("    {}", signatures_deck().get(slot))
}

fn func_name(signature: &str) -> &str {
    signature
        .split('(')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .last()
        .unwrap_or("load")
}

fn blank(width: usize) -> Line {
    chrome(width, &[("", FG)])
}

fn tool(width: usize, name: &str, arg: &str) -> Line {
    chrome(
        width,
        &[
            (&format!("{BULLET} "), GREEN),
            (name, &format!("bold {FG}")),
            (&format!("({arg})"), FG),
        ],
    )
}

fn result(width: usize, first: bool, segments: Vec<Segment>) -> Line {
    let mut all = vec![seg(if first { ELBOW } else { CONT }, DIM)];
    all.extend(segments);
    chrome_owned(width, all)
}

fn context(width: usize, n: usize, line: &str) -> Line {
    let mut all = vec![seg(format!("{n:>7} "), DIM), seg("  ", FG)];
    all.extend(code(line));
    chrome_owned(width, all)
}

fn changed(width: usize, n: usize, sign: &str, colour: &str, bg: &str, line: &str) -> Line {
    let segs = crop(
        vec![
            seg(format!("{n:>7} "), DIM),
            seg(format!("{sign} "), format!("{colour} {bg}")),
            seg(line, format!("{FG} {bg}")),
        ],
        width,
    );
    Line {
        segments: pad(segs, width, bg),
        is_book: false,
        pos: None,
    }
}

fn edit_head(width: usize, path: &str, adds: usize, rems: usize) -> Vec<Line> {
    let summary = format!(
        " with {} and {}",
        plural(adds, "addition"),
        plural(rems, "removal")
    );
    vec![
        tool(width, "Update", path),
        result(
            width,
            true,
            vec![
                seg("Updated ", FG),
                seg(path, format!("bold {FG}")),
                seg(summary, FG),
            ],
        ),
    ]
}

#[derive(Default)]
pub struct AgentMask {
    /// Slot (line number) where the filler after the last paragraph starts.
    next: usize,
    /// Pending lines of the hidden stream.
    stream: VecDeque<Line>,
    stream_n: usize,
    last_unit: &'static str,
}

impl AgentMask {
    fn pick_kind(&mut self, kinds: &[&'static str], rng: &mut Rng) -> &'static str {
        let choices: Vec<&'static str> = kinds
            .iter()
            .copied()
            .filter(|k| *k != self.last_unit)
            .collect();
        let choices = if choices.is_empty() {
            kinds.to_vec()
        } else {
            choices
        };
        self.last_unit = *pick(rng, &choices);
        self.last_unit
    }

    /// One whole tool call, with its result and a blank line after it.
    fn unit(&self, kind: &str, slot: usize, rng: &mut Rng, width: usize) -> Vec<Line> {
        let files = files_deck();
        match kind {
            "read" => vec![
                tool(width, "Read", files.get(slot)),
                result(
                    width,
                    true,
                    vec![
                        seg("Read ", FG),
                        seg(randint(rng, 40, 480).to_string(), format!("bold {FG}")),
                        seg(" lines", FG),
                    ],
                ),
                blank(width),
            ],
            "search" => {
                let pattern = func_name(signatures_deck().get(slot));
                vec![
                    tool(
                        width,
                        "Search",
                        &format!("pattern: \"{pattern}\", path: \"{}\"", files.get(slot)),
                    ),
                    result(
                        width,
                        true,
                        vec![
                            seg("Found ", FG),
                            seg(randint(rng, 2, 14).to_string(), format!("bold {FG}")),
                            seg(" lines", FG),
                        ],
                    ),
                    blank(width),
                ]
            }
            "bash" => {
                let cmd = commands_deck().get(slot);
                let mut out = vec![tool(width, "Bash", cmd)];
                out.extend(self.output(cmd, slot, rng, width));
                out.push(blank(width));
                out
            }
            "todos" => {
                let n = randint(rng, 2, 3);
                let mut out = vec![chrome(
                    width,
                    &[
                        (&format!("{BULLET} "), GREEN),
                        ("Update Todos", &format!("bold {FG}")),
                    ],
                )];
                for k in 0..n {
                    let (mark, style) = if k < n - 1 {
                        ("☒ ", DIM)
                    } else {
                        ("☐ ", FG)
                    };
                    out.push(result(
                        width,
                        k == 0,
                        vec![seg(mark, style), seg(tasks_deck().get(slot * 3 + k), style)],
                    ));
                }
                out.push(blank(width));
                out
            }
            _ => {
                // a small edit with no text in it
                let n = 20 + slot * 7 % 300;
                let mut out = edit_head(width, files.get(slot), 1, 1);
                out.push(context(width, n, &signature(slot)));
                out.push(changed(width, n + 1, "-", RED, DEL_BG, &stmt(slot, 0)));
                out.push(changed(width, n + 1, "+", GREEN, ADD_BG, &stmt(slot, 1)));
                out.push(blank(width));
                out
            }
        }
    }

    fn output(&self, cmd: &str, slot: usize, rng: &mut Rng, width: usize) -> Vec<Line> {
        let files = files_deck();
        if cmd.starts_with("pytest") || cmd.starts_with("python -m pytest") {
            let passed = randint(rng, 3, 60);
            let secs = randint(rng, 8, 300) as f64 / 100.0;
            return vec![
                result(
                    width,
                    true,
                    vec![seg(".".repeat(passed.min(40)), GREEN), seg("  [100%]", FG)],
                ),
                result(
                    width,
                    false,
                    vec![
                        seg(format!("{passed} passed"), GREEN),
                        seg(format!(" in {secs:.2}s"), FG),
                    ],
                ),
            ];
        }
        if cmd.starts_with("ruff") {
            return vec![result(width, true, vec![seg("All checks passed!", FG)])];
        }
        if cmd.starts_with("mypy") {
            return vec![result(
                width,
                true,
                vec![seg("Success: no issues found in 1 source file", GREEN)],
            )];
        }
        if cmd.starts_with("rg") {
            let n = randint(rng, 1, 2);
            return (0..n)
                .map(|k| {
                    let at = format!("{}:{}:", files.get(slot + k), randint(rng, 10, 400));
                    result(
                        width,
                        k == 0,
                        vec![
                            seg(at, DIM),
                            seg(signatures_deck().get(slot + k).trim(), FG),
                        ],
                    )
                })
                .collect();
        }
        if cmd.starts_with("git log") {
            let n = randint(rng, 1, 3);
            return (0..n)
                .map(|k| {
                    let sha = format!("{:07x} ", rng.gen_range(0..1u32 << 28));
                    result(
                        width,
                        k == 0,
                        vec![seg(sha, "#d29922"), seg(tasks_deck().get(slot * 3 + k), FG)],
                    )
                })
                .collect();
        }
        // git diff --stat
        let k = randint(rng, 1, 2);
        let mut rows: Vec<Line> = (0..k)
            .map(|j| {
                let (a, d) = (randint(rng, 1, 30), randint(rng, 0, 12));
                result(
                    width,
                    j == 0,
                    vec![
                        seg(format!(" {} | {:>3} ", files.get(slot + j), a + d), FG),
                        seg("+".repeat(a.min(12)), GREEN),
                        seg("-".repeat(d.min(8)), RED),
                    ],
                )
            })
            .collect();
        rows.push(result(
            width,
            false,
            vec![seg(format!(" {k} files changed"), FG)],
        ));
        rows
    }
}

impl Mask for AgentMask {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        vec![
            chrome(width, &[("> refactor the cache layer and add tests", DIM)]),
            blank(width),
        ]
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        let lay = layout(&para.text);
        match lay.kind {
            Kind::Say => ragged(&para.text, width.saturating_sub(2).max(1)).len() + 1,
            Kind::Edit => {
                2 + lay.before + lay.removed + ragged(&para.text, body_width(width)).len() + 1 + 1
            }
        }
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        let lay = layout(&para.text);
        let slot = line_no_start;
        if lay.kind == Kind::Say {
            let mut out: Vec<Line> = ragged(&para.text, width.saturating_sub(2).max(1))
                .into_iter()
                .enumerate()
                .map(|(k, text)| {
                    let lead = if k == 0 {
                        format!("{BULLET} ")
                    } else {
                        "  ".into()
                    };
                    Line::book(vec![seg(lead, FG), seg(text, format!("book {FG}"))], pos)
                })
                .collect();
            out.push(blank(width));
            self.next = slot + out.len();
            return out;
        }

        let texts = ragged(&para.text, body_width(width));
        let first = 20 + (line_no_start * 13) % 380;
        let mut out = edit_head(width, files_deck().get(slot), texts.len(), lay.removed);
        let mut n = first;
        out.push(context(width, n, &signature(slot)));
        n += 1;
        for k in 1..lay.before {
            out.push(context(width, n, &stmt(slot, k)));
            n += 1;
        }
        for k in 0..lay.removed {
            out.push(changed(width, n + k, "-", RED, DEL_BG, &stmt(slot, 2 + k)));
        }
        let ind = " ".repeat(indent(width));
        for (k, text) in texts.iter().enumerate() {
            let segs = vec![
                seg(format!("{:>7} ", n + k), DIM),
                seg("+ ", format!("{GREEN} {ADD_BG}")),
                seg(ind.clone(), ADD_BG),
                seg(text.clone(), format!("book {FG} {ADD_BG}")),
            ];
            out.push(Line::book(pad(segs, width, ADD_BG), pos));
        }
        out.push(context(width, n + texts.len(), &stmt(slot, 7)));
        out.push(blank(width));
        self.next = slot + out.len();
        out
    }

    /// Exactly `count` lines of whole tool calls; leftover rows become a thinking line or a gap.
    fn noise_block(&mut self, count: usize, rng: &mut Rng, width: usize) -> Option<Vec<Line>> {
        let mut out: Vec<Line> = vec![];
        self.last_unit = "";
        for _ in 0..20 {
            if out.len() >= count {
                break;
            }
            let kind = self.pick_kind(&["read", "search", "bash", "todos", "edit"], rng);
            // a unit's slot is the line number it starts on: unique, like the text's own
            let unit = self.unit(kind, self.next + out.len(), rng, width);
            if unit.len() <= count - out.len() {
                out.extend(unit);
            }
        }
        // rows no whole unit fits into go first, so they follow the blank line ending the
        // paragraph above instead of sitting right on top of the next one
        let left = count - out.len();
        let mut lead = vec![];
        if left > 0 {
            lead.push(chrome(width, &[(THINKING, DIM)]));
            lead.extend((1..left).map(|_| blank(width)));
        }
        lead.extend(out);
        Some(lead)
    }

    /// Next line of an endless session, for the hidden screen.
    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        if self.stream.is_empty() {
            let kind = self.pick_kind(&["read", "search", "bash", "todos", "edit", "edit"], rng);
            let unit = self.unit(kind, STREAM_BASE + self.stream_n, rng, width);
            if random(rng) < 0.15 {
                self.stream.push_back(chrome(width, &[(THINKING, DIM)]));
            }
            self.stream.extend(unit);
            self.stream_n += 1;
        }
        self.stream.pop_front().expect("a unit is never empty")
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        let tokens = done as f64 * 3.7;
        format!(
            "✻ Working… ({done}/{total} · ↓ {:.1}k tokens)",
            tokens / 1000.0
        )
    }

    fn animated_label(
        &self,
        done: usize,
        total: usize,
        width: usize,
        elapsed: Duration,
    ) -> Option<String> {
        let head = format!("{} {}…", spinner_frame(elapsed), verb(elapsed));
        let time = elapsed_text(elapsed);
        let tokens = tokens_text(done, elapsed);
        // drop the least useful parts first when the window is narrow
        let tries = [
            format!("{head} ({time} · {done}/{total} · ↓ {tokens} tokens)"),
            format!("{head} ({time} · {done}/{total})"),
            format!("{head} ({done}/{total})"),
        ];
        let fits = |s: &&String| s.chars().count() <= width;
        Some(tries.iter().find(fits).unwrap_or(&tries[2]).clone())
    }

    fn typewriter(&self) -> bool {
        true
    }
}

/// Spinner glyphs, grown then shrunk again like Claude Code's.
pub const SPINNER: &[&str] = &["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
/// How long each spinner glyph shows.
pub const SPIN_FRAME: Duration = Duration::from_millis(120);
/// What the agent says it is doing; one at a time, a few seconds each.
pub const VERBS: &[&str] = &[
    "Working",
    "Thinking",
    "Brewing",
    "Reticulating",
    "Pondering",
    "Computing",
    "Cogitating",
    "Crafting",
    "Mulling",
    "Percolating",
    "Synthesizing",
    "Noodling",
    "Tinkering",
    "Forging",
    "Deliberating",
    "Ruminating",
    "Conjuring",
    "Simmering",
    "Untangling",
    "Wrangling",
    "Calibrating",
    "Assembling",
    "Distilling",
    "Refactoring",
];
/// How long one verb stays.
pub const VERB_SPAN: Duration = Duration::from_secs(4);
/// Tokens "received" per second, on top of the ones the progress accounts for.
const TOKENS_PER_SEC: f64 = 42.0;
const TOKENS_PER_PARA: f64 = 117.0;

/// The spinner glyph `elapsed` into the session.
pub fn spinner_frame(elapsed: Duration) -> &'static str {
    let n = elapsed.as_millis() / SPIN_FRAME.as_millis();
    SPINNER[(n % SPINNER.len() as u128) as usize]
}

/// The verb `elapsed` into the session: a step coprime with the list length
/// visits every verb and never repeats one twice in a row.
pub fn verb(elapsed: Duration) -> &'static str {
    let slot = (elapsed.as_secs() / VERB_SPAN.as_secs()) as usize;
    VERBS[(slot * 7 + 2) % VERBS.len()]
}

/// `12s`, `3m 4s`, `1h 2m`.
fn elapsed_text(elapsed: Duration) -> String {
    let s = elapsed.as_secs();
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {}s", s / 60, s % 60),
        _ => format!("{}h {}m", s / 3600, s / 60 % 60),
    }
}

/// Tokens so far: they grow with the paragraphs passed and with time.
fn tokens_text(done: usize, elapsed: Duration) -> String {
    let n = done as f64 * TOKENS_PER_PARA + elapsed.as_secs_f64() * TOKENS_PER_SEC;
    if n < 1000.0 {
        format!("{}", n as u64)
    } else {
        format!("{:.1}k", n / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use std::collections::HashMap;

    fn para_of_kind(kind: Kind, start: usize) -> Paragraph {
        (start..start + 500)
            .map(|i| {
                format!("Sabah {i} erkenden uyandı, pencereyi açtı ve dışarı baktı. ").repeat(3)
            })
            .find(|t| layout(t).kind == kind)
            .map(Paragraph::new)
            .expect("found")
    }

    fn rng(seed: u64) -> Rng {
        Rng::seed_from_u64(seed)
    }

    /// A numbered diff line: a line number, then `+ `, `- ` or two spaces, then code.
    fn is_code_line(p: &str) -> bool {
        let c: Vec<char> = p.chars().collect();
        c.len() > 10
            && c[..7]
                .iter()
                .collect::<String>()
                .trim()
                .parse::<usize>()
                .is_ok()
            && c[7] == ' '
            && matches!((c[8], c[9]), ('+' | '-' | ' ', ' '))
    }

    fn has_bg(l: &Line, bg: &str) -> bool {
        l.segments.iter().any(|(_, s)| s.contains(bg))
    }

    // -- test_agent.py --

    #[test]
    fn layout_is_picked_from_the_text_and_mixes_kinds() {
        let edits = (0..200)
            .filter(|i| layout(&format!("paragraf {i}")).kind == Kind::Edit)
            .count();
        assert!((110..170).contains(&edits), "{edits}");
        assert_eq!(layout("aynı metin"), layout("aynı metin"));
    }

    #[test]
    fn edit_paragraph_is_a_diff_with_the_text_as_added_lines() {
        let p = para_of_kind(Kind::Edit, 0);
        let lines = AgentMask::default().render(&p, (0, 0), 100, &mut rng(1), 40);
        assert!(lines[0].plain().starts_with(&format!("{BULLET} Update(")));
        assert!(lines[1].plain().contains("⎿  Updated ") && lines[1].plain().contains(" with "));
        let added: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(!added.is_empty());
        assert!(
            added.iter().all(
                |l| l.plain().chars().skip(8).take(2).collect::<String>() == "+ "
                    && has_bg(l, ADD_BG)
            )
        );
        assert_eq!(
            lines.iter().filter(|l| has_bg(l, DEL_BG)).count(),
            layout(&p.text).removed
        );
        assert_eq!(lines.last().unwrap().plain(), "");
        let summary = lines[1].plain();
        assert!(
            summary.contains(&format!("{} addition", added.len())),
            "{summary}"
        );
        assert!(
            summary.contains(&format!("{} removal", layout(&p.text).removed)),
            "{summary}"
        );
    }

    #[test]
    fn say_paragraph_is_narration() {
        let lines =
            AgentMask::default().render(&para_of_kind(Kind::Say, 0), (0, 0), 100, &mut rng(1), 40);
        let text: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
        assert!(text[0].plain().starts_with(&format!("{BULLET} ")));
        assert!(text[1..].iter().all(|l| l.plain().starts_with("  ")));
        assert_eq!(lines.last().unwrap().plain(), "");
    }

    #[test]
    fn same_paragraph_draws_the_same_every_time() {
        let p = para_of_kind(Kind::Edit, 0);
        let a = AgentMask::default().render(&p, (0, 3), 90, &mut rng(5), 120);
        let b = AgentMask::default().render(&p, (0, 3), 90, &mut rng(5), 120);
        assert_eq!(a, b);
    }

    #[test]
    fn diff_code_does_not_repeat_across_fifty_paragraphs() {
        let mut m = AgentMask::default();
        let mut seen: Vec<String> = vec![];
        let mut start = 1;
        for i in 0..50 {
            let lines = m.render(
                &para_of_kind(Kind::Edit, i * 7),
                (0, i),
                100,
                &mut rng(i as u64),
                start,
            );
            start += lines.len();
            for l in lines.iter().filter(|l| !l.is_book) {
                let p = l.plain();
                if is_code_line(&p) {
                    seen.push(p.chars().skip(10).collect::<String>().trim().to_string());
                }
            }
        }
        let unique: std::collections::HashSet<&String> = seen.iter().collect();
        assert!(seen.len() > 100 && unique.len() == seen.len());
    }

    #[test]
    fn hidden_stream_does_not_repeat_within_300_lines() {
        let mut m = AgentMask::default();
        let mut r = rng(9);
        let mut last: HashMap<String, usize> = HashMap::new();
        for i in 0..3000 {
            let p = m.noise(&mut r, 100).plain();
            let s = p.trim();
            let meaningful = (s.starts_with(BULLET) && s.contains('('))
                || is_code_line(&p)
                || s.contains('☐')
                || s.contains('☒');
            if meaningful && let Some(prev) = last.insert(s.to_string(), i) {
                assert!(i - prev > 300, "{s} after {}", i - prev);
            }
        }
    }

    #[test]
    fn noise_block_has_exact_size_and_whole_units() {
        for count in 0..=10 {
            let mut m = AgentMask::default();
            m.render(&para_of_kind(Kind::Edit, 0), (0, 0), 100, &mut rng(1), 40);
            let block = m.noise_block(count, &mut rng(count as u64), 100).unwrap();
            assert_eq!(block.len(), count);
            assert!(!block.iter().any(|l| l.is_book));
            if let Some(first) = block.first() {
                assert!(!first.plain().trim_start().starts_with('⎿')); // a result always follows its call
            }
        }
    }

    #[test]
    fn lines_fit_and_diff_background_spans_the_width() {
        let lines =
            AgentMask::default().render(&para_of_kind(Kind::Edit, 0), (0, 0), 80, &mut rng(1), 40);
        assert!(lines.iter().all(|l| l.plain().width() <= 80));
        assert!(
            lines
                .iter()
                .filter(|l| l.is_book)
                .all(|l| l.plain().width() == 80)
        );
    }

    #[test]
    fn wide_text_diff_background_spans_the_width() {
        let text = |i: usize| {
            format!("{i} 漢字かな交じり文の例です。🙂 これは幅の広い文字を含みます。").repeat(4)
        };
        let para = (0..500)
            .map(text)
            .find(|t| layout(t).kind == Kind::Edit)
            .map(Paragraph::new)
            .expect("found");
        for width in [40, 60, 80] {
            let lines = AgentMask::default().render(&para, (0, 0), width, &mut rng(1), 40);
            assert!(lines.iter().all(|l| l.plain().width() <= width));
            assert!(
                lines
                    .iter()
                    .filter(|l| l.is_book)
                    .all(|l| l.plain().width() == width),
                "{width}"
            );
        }
    }

    #[test]
    fn header_and_status() {
        let mut m = AgentMask::default();
        assert!(m.header(&mut rng(1), 80)[0].plain().starts_with('>'));
        assert!(m.progress_label(142, 380, 80).contains("142/380"));
    }

    #[test]
    fn spinner_frames_change_with_time() {
        let at = |ms| spinner_frame(Duration::from_millis(ms));
        assert_eq!(at(0), "·");
        assert_eq!(at(119), "·");
        assert_eq!(at(120), "✢");
        let cycle: Vec<&str> = (0..SPINNER.len() as u64).map(|i| at(i * 120)).collect();
        assert_eq!(cycle, SPINNER);
        assert_eq!(at(SPINNER.len() as u64 * 120), "·"); // and round again
        let m = AgentMask::default();
        let label = |ms| {
            m.animated_label(3, 10, 80, Duration::from_millis(ms))
                .unwrap()
        };
        assert_ne!(label(0), label(120));
        assert!(label(0).starts_with("· "));
    }

    #[test]
    fn verbs_are_deterministic_and_change_every_few_seconds() {
        let at = |s| verb(Duration::from_secs(s));
        assert_eq!(at(0), at(3));
        assert_eq!(at(0), verb(Duration::from_millis(3999)));
        let mut seen = std::collections::HashSet::new();
        for slot in 0..VERBS.len() as u64 {
            let (here, next) = (at(slot * 4), at(slot * 4 + 4));
            assert_ne!(here, next);
            seen.insert(here);
        }
        assert_eq!(seen.len(), VERBS.len()); // every verb comes up
        let again: Vec<&str> = (0..50).map(at).collect();
        assert_eq!(again, (0..50).map(at).collect::<Vec<_>>());
    }

    #[test]
    fn animated_label_counts_seconds_and_tokens_and_fits() {
        let m = AgentMask::default();
        let l = m
            .animated_label(5, 380, 80, Duration::from_secs(12))
            .unwrap();
        assert!(l.contains("(12s · 5/380 · ↓ "), "{l}");
        assert!(l.ends_with(" tokens)"), "{l}");
        let tokens = |secs| {
            let l = m
                .animated_label(5, 380, 200, Duration::from_secs(secs))
                .unwrap();
            let t = l.split("↓ ").nth(1).unwrap().split(' ').next().unwrap();
            let k = t
                .strip_suffix('k')
                .map(|n| n.parse::<f64>().unwrap() * 1000.0);
            k.unwrap_or_else(|| t.parse().unwrap())
        };
        assert!(tokens(60) > tokens(1));
        assert!(
            m.animated_label(5, 380, 80, Duration::from_secs(185))
                .unwrap()
                .contains("(3m 5s ·")
        );
        let narrow = m
            .animated_label(5, 380, 30, Duration::from_secs(1))
            .unwrap();
        assert!(narrow.chars().count() <= 30, "{narrow}");
        assert!(narrow.contains("5/380"));
        assert!(!l.contains("esc"));
    }
}
