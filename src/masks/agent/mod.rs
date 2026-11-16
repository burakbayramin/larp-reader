//! A coding-agent session (`masks/agent.py`): tool calls, coloured diffs, command output and narration.
//!
//! Most paragraphs appear as the added lines of an edit, the rest as the
//! agent's own remarks. Filler comes from the shuffled pools in `codegen`,
//! read by a "slot": the line number where a paragraph or filler unit starts.
//! Line numbers are unique along the text, so neighbours read different parts
//! of the pools, and redrawing a paragraph gives the same lines.

mod draw;
mod figure;
mod reply;
mod spinner;
mod subagents;
mod tools;
mod welcome;

use std::collections::VecDeque;
use std::time::Duration;

use super::base::{Line, chrome, seg};
use super::codegen::files_deck;
use super::look::{Look, Role};
use super::palette::{DIM, FG, GREEN, RED};
use super::{Mask, Rng, random, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use draw::{blank, body_width, changed, context, edit_head, indent, pad, signature, stmt};
use spinner::{elapsed_text, tokens_text};
use subagents::agents;
use welcome::welcome;

pub use figure::{SHOW_EVERY, SHOW_FOR, SPRITE, STEP_FOR, sprite_frame};
pub use spinner::{SPIN_FRAME, SPINNER, VERB_SPAN, VERBS, spinner_frame, verb};

pub const BULLET: &str = if cfg!(windows) { "●" } else { "⏺" };
pub const ADD_BG: Role = Role::AddedBackground;
pub const DEL_BG: Role = Role::RemovedBackground;
const THINKING: &str = "✻ Thinking…";
/// The hidden stream reads a part of the pools the text never reaches.
const STREAM_BASE: usize = 10_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The text as added lines of an edit.
    Edit,
    /// The text as narration.
    Say,
    /// The text as the summary after a few sub-agents ran in parallel.
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub kind: Kind,
    /// Lines the edit takes out.
    pub removed: usize,
    /// Unchanged lines shown above the change.
    pub before: usize,
    /// Sub-agents listed above a `Team` summary.
    pub agents: usize,
}

/// How a paragraph is laid out; from its text alone, so it can be counted without drawing.
pub fn layout(text: &str) -> Layout {
    let h = crc32fast::hash(text.as_bytes());
    let kind = match h % 10 {
        0..3 if (h >> 16).is_multiple_of(2) => Kind::Team,
        0..3 => Kind::Say,
        _ => Kind::Edit,
    };
    Layout {
        kind,
        removed: (h >> 4) as usize % 4,
        before: 1 + (h >> 8) as usize % 2,
        agents: 2 + (h >> 12) as usize % 2,
    }
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

impl Mask for AgentMask {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        let mut out = welcome(width);
        out.push(chrome(
            width,
            &[("> refactor the cache layer and add tests", DIM)],
        ));
        out.push(blank(width));
        out
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        let lay = layout(&para.text);
        match lay.kind {
            Kind::Say => ragged_count(&para.text, width.saturating_sub(2).max(1)) + 1,
            Kind::Team => {
                1 + lay.agents + ragged_count(&para.text, width.saturating_sub(2).max(1)) + 1
            }
            Kind::Edit => {
                2 + lay.before + lay.removed + ragged_count(&para.text, body_width(width)) + 1 + 1
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
        if lay.kind != Kind::Edit {
            let mut out = if lay.kind == Kind::Team {
                agents(width, slot, lay.agents, false)
            } else {
                vec![]
            };
            let said: Vec<Line> = ragged(&para.text, width.saturating_sub(2).max(1))
                .into_iter()
                .enumerate()
                .map(|(k, text)| {
                    let lead = if k == 0 {
                        format!("{BULLET} ")
                    } else {
                        "  ".into()
                    };
                    Line::book(vec![seg(lead, FG), seg(text, FG.book())], pos)
                })
                .collect();
            out.extend(said);
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
                seg("+ ", GREEN.on(ADD_BG)),
                seg(ind.clone(), Look::PLAIN.on(ADD_BG)),
                seg(text.clone(), FG.on(ADD_BG).book()),
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
            let kind = self.pick_kind(
                &["read", "search", "bash", "todos", "edit", "agents", "write"],
                rng,
            );
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
            let kind = self.pick_kind(
                &[
                    "read", "search", "bash", "todos", "edit", "edit", "agents", "write",
                ],
                rng,
            );
            let unit = self.unit(kind, STREAM_BASE + self.stream_n, rng, width);
            if random(rng) < 0.15 {
                self.stream.push_back(chrome(width, &[(THINKING, DIM)]));
            }
            self.stream.extend(unit);
            self.stream_n += 1;
        }
        self.stream.pop_front().expect("a unit is never empty")
    }

    /// The user asking for the next thing.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let a = section::name(chapter, 0);
        let b = section::name(chapter, 1);
        let ask = match chapter % 5 {
            0 => format!("> next, add retries to the {a} client"),
            1 => format!("> now move {a} parsing into its own module"),
            2 => format!("> next, write tests for {a} and {b}"),
            3 => format!("> ok, now clean up the {a} config"),
            _ => format!("> next, make {a} handle an empty {b}"),
        };
        vec![chrome(width, &[(&ask, DIM)]), blank(width)]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        let tokens = done as f64 * 3.7;
        format!(
            "✻ Working… ({done}/{total} · ↓ {:.1}k tokens)",
            tokens / 1000.0
        )
    }

    fn status_right(
        &self,
        done: usize,
        total: usize,
        elapsed: std::time::Duration,
    ) -> Vec<super::base::Segment> {
        let _ = (total, elapsed);
        let edits = 3 + done / 4;
        vec![
            super::base::seg("▸▸ accept edits on", super::palette::CONTROL),
            super::base::seg("  ·  ", super::palette::DIM),
            super::base::seg("⎇ main", super::palette::GREEN),
            super::base::seg(format!(" +{edits}"), super::palette::YELLOW),
        ]
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

    fn sprite(&self, elapsed: Duration, active: bool) -> Option<Vec<(&'static str, &'static str)>> {
        Some(sprite_frame(elapsed, active))
    }

    fn prompt_box(&self) -> Option<&'static str> {
        Some("Try \"add tests for the retry path\"")
    }

    fn reply(&self, request: &str, width: usize) -> Vec<Vec<Line>> {
        self.reply_groups(request, width)
    }
}

#[cfg(test)]
mod tests;
