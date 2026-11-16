//! The answer to a request typed into the prompt box.

use super::super::base::{Line, chrome};
use super::super::palette::{DIM, FG};
use super::super::{pick, randint, seeded};
use super::draw::{blank, plural};
use super::subagents::agents;
use super::{AgentMask, BULLET, THINKING};
use crate::text::ragged;

/// Replies to typed requests read yet another part.
const REPLY_BASE: usize = 20_000_000;
/// How a reply ends; `{n}` is how many files it touched.
const SUMMARIES: &[&str] = &[
    "Done. Updated {n} and all tests pass.",
    "All set: {n} changed, the suite is green.",
    "Finished. The change touches {n}; tests and lint pass.",
    "Done. I made the change in {n} and ran the tests: all passing.",
    "That's in. {n} updated, nothing else needed.",
    "Done. {n} edited; existing behaviour is unchanged and covered by tests.",
];

/// Closing lines for a reply that changed nothing.
pub(super) const NO_CHANGE: &[&str] = &[
    "Looked into it: the current code already handles this, so nothing needs to change.",
    "Checked it end to end. No change needed; the tests already cover this case.",
    "Nothing to change here: it works as asked and the suite is green.",
];

impl AgentMask {
    /// Look at the code, do a few things, sum up: 2-5 tool calls picked by
    /// the request's hash, with a batch of sub-agents for longer requests.
    pub(super) fn reply_groups(&self, request: &str, width: usize) -> Vec<Vec<Line>> {
        let request = request.trim();
        let h = crc32fast::hash(request.as_bytes()) as usize;
        let mut rng = seeded(&format!("reply {request}"));
        let slot = REPLY_BASE + h % 100_000 * 64;
        let echo: Vec<Line> = ragged(request, width.saturating_sub(2).max(1))
            .into_iter()
            .enumerate()
            .map(|(k, t)| {
                let lead = if k == 0 { "> " } else { "  " };
                chrome(width, &[(lead, DIM), (&t, DIM)])
            })
            .chain([blank(width)])
            .collect();
        let mut out = vec![echo, vec![chrome(width, &[(THINKING, DIM)]), blank(width)]];
        let long = request.split_whitespace().count() > 8;
        let n = 2 + h % 3 + usize::from(long);
        let mut files = 0;
        let mut last = "";
        for k in 0..n {
            let kind = if k == 0 {
                *pick(&mut rng, &["read", "search"])
            } else if k == 1 && long {
                "agents"
            } else if k + 1 == n {
                *pick(&mut rng, &["bash", "edit"])
            } else {
                let kinds: Vec<&str> = ["read", "search", "edit", "write", "todos", "bash"]
                    .into_iter()
                    .filter(|x| *x != last)
                    .collect();
                *pick(&mut rng, &kinds)
            };
            last = kind;
            let at = slot + k * 8;
            out.push(match kind {
                // a finished batch: the reply waits for nothing
                "agents" => {
                    let mut lines = agents(width, at, randint(&mut rng, 2, 4), false);
                    lines.push(blank(width));
                    lines
                }
                _ => self.unit(kind, at, &mut rng, width),
            });
            files += usize::from(matches!(kind, "edit" | "write"));
        }
        let summary = if files == 0 {
            NO_CHANGE[h / 3 % NO_CHANGE.len()].to_string()
        } else {
            SUMMARIES[h / 3 % SUMMARIES.len()].replace("{n}", &plural(files, "file"))
        };
        out.push(vec![
            chrome(width, &[(&format!("{BULLET} "), FG), (&summary, FG)]),
            blank(width),
        ]);
        out
    }
}
