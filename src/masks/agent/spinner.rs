//! The status line's spinner, verb, elapsed time and token count.

use std::time::Duration;

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
pub(super) fn elapsed_text(elapsed: Duration) -> String {
    let s = elapsed.as_secs();
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {}s", s / 60, s % 60),
        _ => format!("{}h {}m", s / 3600, s / 60 % 60),
    }
}

/// Tokens so far: they grow with the paragraphs passed and with time.
pub(super) fn tokens_text(done: usize, elapsed: Duration) -> String {
    let n = done as f64 * TOKENS_PER_PARA + elapsed.as_secs_f64() * TOKENS_PER_SEC;
    if n < 1000.0 {
        format!("{}", n as u64)
    } else {
        format!("{:.1}k", n / 1000.0)
    }
}
