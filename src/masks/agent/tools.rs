//! Whole tool calls with their results: the filler between paragraphs.

use rand::Rng as _;

use super::super::base::{Line, chrome, seg};
use super::super::codegen::{commands_deck, files_deck, signatures_deck, tasks_deck};
use super::super::palette::{DIM, FG, GREEN, RED, YELLOW};
use super::super::{Rng, pick, randint};
use super::draw::{blank, changed, context, edit_head, func_name, result, signature, stmt, tool};
use super::subagents::agents;
use super::{ADD_BG, AgentMask, BULLET, DEL_BG};

impl AgentMask {
    pub(super) fn pick_kind(&mut self, kinds: &[&'static str], rng: &mut Rng) -> &'static str {
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
    pub(super) fn unit(&self, kind: &str, slot: usize, rng: &mut Rng, width: usize) -> Vec<Line> {
        let files = files_deck();
        match kind {
            "read" => vec![
                tool(width, "Read", files.get(slot)),
                result(
                    width,
                    true,
                    vec![
                        seg("Read ", FG),
                        seg(randint(rng, 40, 480).to_string(), FG.bold()),
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
                            seg(randint(rng, 2, 14).to_string(), FG.bold()),
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
            "agents" => {
                let n = randint(rng, 2, 4);
                let mut out = agents(width, slot, n, true);
                out.push(result(
                    width,
                    true,
                    vec![seg("Waiting for agents to finish…", DIM)],
                ));
                out.push(blank(width));
                out
            }
            "write" => {
                let path = files_deck().get(slot + 1);
                vec![
                    tool(width, "Write", path),
                    result(
                        width,
                        true,
                        vec![
                            seg("Wrote ", FG),
                            seg(randint(rng, 12, 160).to_string(), FG.bold()),
                            seg(" lines to ", FG),
                            seg(path, FG.bold()),
                        ],
                    ),
                    blank(width),
                ]
            }
            "todos" => {
                let n = randint(rng, 2, 3);
                let mut out = vec![chrome(
                    width,
                    &[(&format!("{BULLET} "), GREEN), ("Update Todos", FG.bold())],
                )];
                for k in 0..n {
                    let (mark, look) = if k < n - 1 {
                        ("☒ ", DIM)
                    } else {
                        ("☐ ", FG)
                    };
                    out.push(result(
                        width,
                        k == 0,
                        vec![seg(mark, look), seg(tasks_deck().get(slot * 3 + k), look)],
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

    pub(super) fn output(&self, cmd: &str, slot: usize, rng: &mut Rng, width: usize) -> Vec<Line> {
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
                        vec![seg(sha, YELLOW), seg(tasks_deck().get(slot * 3 + k), FG)],
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
