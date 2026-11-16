//! The pieces every tool call is drawn from: bullets, results, numbered diff lines.

use super::super::base::{Line, Segment, chrome, chrome_owned, crop, seg};
use super::super::codegen::{signatures_deck, statements_deck};
use super::super::look::{Look, Role};
use super::super::palette::{DIM, FG, GREEN, highlight};
use super::BULLET;
use unicode_width::UnicodeWidthStr;

/// First line of a tool result, and its following lines.
const ELBOW: &str = "  ⎿  ";
const CONT: &str = "     ";
/// "%7d " before the +/- sign.
const GUTTER: usize = 8;
/// Sign and a space.
const SIGN: usize = 2;
/// Statements sit inside a method.
const BODY_INDENT: usize = 8;
/// Statements one slot may use.
const PER_SLOT: usize = 8;

fn code(line: &str) -> Vec<Segment> {
    if line.trim().is_empty() {
        vec![]
    } else {
        highlight("python", line).swap_remove(0)
    }
}

pub(super) fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

pub(super) fn indent(width: usize) -> usize {
    BODY_INDENT.min(width.saturating_sub(GUTTER + SIGN) / 10)
}

pub(super) fn body_width(width: usize) -> usize {
    width.saturating_sub(GUTTER + SIGN + indent(width)).max(1)
}

/// Fill the line out to `width` with the background colour.
pub(super) fn pad(mut segments: Vec<Segment>, width: usize, bg: Role) -> Vec<Segment> {
    let used: usize = segments.iter().map(|(t, _)| t.width()).sum();
    if used < width {
        segments.push(seg(" ".repeat(width - used), Look::PLAIN.on(bg)));
    }
    segments
}

pub(super) fn stmt(slot: usize, j: usize) -> String {
    format!(
        "{}{}",
        " ".repeat(BODY_INDENT),
        statements_deck().get(slot * PER_SLOT + j)
    )
}

pub(super) fn signature(slot: usize) -> String {
    format!("    {}", signatures_deck().get(slot))
}

pub(super) fn func_name(signature: &str) -> &str {
    signature
        .split('(')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .last()
        .unwrap_or("load")
}

pub(super) fn blank(width: usize) -> Line {
    chrome(width, &[("", FG)])
}

pub(super) fn tool(width: usize, name: &str, arg: &str) -> Line {
    chrome(
        width,
        &[
            (&format!("{BULLET} "), GREEN),
            (name, FG.bold()),
            (&format!("({arg})"), FG),
        ],
    )
}

pub(super) fn result(width: usize, first: bool, segments: Vec<Segment>) -> Line {
    let mut all = vec![seg(if first { ELBOW } else { CONT }, DIM)];
    all.extend(segments);
    chrome_owned(width, all)
}

pub(super) fn context(width: usize, n: usize, line: &str) -> Line {
    let mut all = vec![seg(format!("{n:>7} "), DIM), seg("  ", FG)];
    all.extend(code(line));
    chrome_owned(width, all)
}

pub(super) fn changed(
    width: usize,
    n: usize,
    sign: &str,
    colour: Look,
    bg: Role,
    line: &str,
) -> Line {
    let segs = crop(
        vec![
            seg(format!("{n:>7} "), DIM),
            seg(format!("{sign} "), colour.on(bg)),
            seg(line, FG.on(bg)),
        ],
        width,
    );
    Line {
        segments: pad(segs, width, bg),
        is_book: false,
        pos: None,
    }
}

pub(super) fn edit_head(width: usize, path: &str, adds: usize, rems: usize) -> Vec<Line> {
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
            vec![seg("Updated ", FG), seg(path, FG.bold()), seg(summary, FG)],
        ),
    ]
}
