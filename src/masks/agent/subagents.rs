//! Batches of sub-agents run in parallel.

use super::super::base::{Line, chrome, chrome_owned, seg};
use super::super::codegen::tasks_deck;
use super::super::palette::{DIM, FG, GREEN, YELLOW};
use super::BULLET;

/// What each sub-agent is called.
const ROLES: &[&str] = &["Explore", "Plan", "Review", "Test", "Docs", "Debug"];

/// A batch of sub-agents run in parallel: a header and one tree row each.
/// `running`: still going ("Running 3 agents…"), else finished.
pub(super) fn agents(width: usize, slot: usize, n: usize, running: bool) -> Vec<Line> {
    let head = if running {
        format!("Running {n} agents…")
    } else {
        format!("{n} agents finished")
    };
    let mut out = vec![chrome(
        width,
        &[
            (&format!("{BULLET} "), if running { YELLOW } else { GREEN }),
            (&head, FG.bold()),
        ],
    )];
    for k in 0..n {
        let tree = if k + 1 == n {
            "   └─ "
        } else {
            "   ├─ "
        };
        let uses = 3 + (slot * 7 + k * 5) % 19;
        let tokens = (40 + (slot * 13 + k * 31) % 260) as f64 / 10.0;
        let role = ROLES[(slot + k) % ROLES.len()];
        out.push(chrome_owned(
            width,
            vec![
                seg(tree, DIM),
                seg(role, FG.bold()),
                seg(": ", DIM),
                seg(tasks_deck().get(slot * 3 + k), FG),
                seg(format!(" · {uses} tool uses · {tokens:.1}k tokens"), DIM),
            ],
        ));
    }
    out
}
