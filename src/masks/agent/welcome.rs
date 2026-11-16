//! The boxed welcome at the top of a session.

use super::super::base::{Line, chrome_owned, seg};
use super::super::palette::{DIM, FG, YELLOW};
use super::draw::blank;
use super::figure::SPRITE;
use unicode_width::UnicodeWidthStr;

/// The version the welcome box names; fixed under test so recorded screens
/// hold across releases.
#[cfg(not(test))]
const SHOWN_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(test)]
const SHOWN_VERSION: &str = "0.0.0";

/// The welcome box needs this many cells; narrower screens go without it.
pub(super) const WELCOME_MIN: usize = 34;
const WELCOME_MAX: usize = 58;

/// A boxed welcome like an interactive session's: the figure and four
/// lines beside it. Empty on narrow screens.
pub(super) fn welcome(width: usize) -> Vec<Line> {
    if width < WELCOME_MIN {
        return vec![];
    }
    let w = width.min(WELCOME_MAX);
    let room = w - 4 - SPRITE[0].width() - 2;
    let version = format!("larpr v{SHOWN_VERSION}");
    let texts = [
        (version.as_str(), FG.bold()),
        ("role: senior engineer (self-appointed)", DIM),
        ("cwd: ~/work/api", DIM),
        ("armor: cardboard, sword: foam", DIM),
    ];
    let mut out = vec![chrome_owned(
        width,
        vec![seg(format!("╭{}╮", "─".repeat(w - 2)), DIM)],
    )];
    for (row, (text, look)) in SPRITE.iter().zip(texts) {
        let text: String = text.chars().take(room).collect();
        let pad = room - text.chars().count();
        out.push(chrome_owned(
            width,
            vec![
                seg("│ ", DIM),
                seg(*row, YELLOW),
                seg("  ", FG),
                seg(text, look),
                seg(format!("{} │", " ".repeat(pad)), DIM),
            ],
        ));
    }
    out.push(chrome_owned(
        width,
        vec![seg(format!("╰{}╯", "─".repeat(w - 2)), DIM)],
    ));
    out.push(blank(width));
    out
}
