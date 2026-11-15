//! Reading speed in words per minute: auto-scroll delays and learning from manual paging (`pace.py`).

pub const DEFAULT_WPM: u32 = 220;
pub const MIN_WPM: u32 = 60;
pub const MAX_WPM: u32 = 1500;
const WPM_STEP: f64 = 1.1;
/// Seconds; lines without text still take a beat.
pub const MIN_DELAY: f64 = 0.12;

// a screen counts as a reading sample only within these bounds
const MIN_SAMPLE_SECONDS: f64 = 2.0; // quicker than this is skimming or paging past
const MAX_SAMPLE_SECONDS: f64 = 300.0; // longer than this the reader was probably away
const MIN_SAMPLE_WORDS: usize = 15;
const LEARN_WEIGHT: f64 = 0.25; // how far one screen moves the estimate toward its own rate

/// A usable rate from any number; the default when it isn't finite.
pub fn clamp_wpm(value: f64) -> u32 {
    if !value.is_finite() {
        return DEFAULT_WPM;
    }
    value.round().clamp(MIN_WPM as f64, MAX_WPM as f64) as u32
}

/// One `+`/`-` step: about 10% faster or slower.
pub fn step_wpm(wpm: u32, direction: i32) -> u32 {
    let factor = if direction > 0 {
        WPM_STEP
    } else {
        1.0 / WPM_STEP
    };
    clamp_wpm(wpm as f64 * factor)
}

/// Words of text (not chrome or filler) in the given lines.
pub fn words_in(lines: &[crate::masks::Line]) -> usize {
    lines
        .iter()
        .filter(|l| l.is_book)
        .flat_map(|l| &l.segments)
        .filter(|(_, look)| look.book)
        .map(|(text, _)| text.split_whitespace().count())
        .sum()
}

/// Seconds to leave a line of `words` words before the next one appears.
pub fn line_delay(words: usize, wpm: u32) -> f64 {
    (words as f64 * 60.0 / wpm.max(1) as f64).max(MIN_DELAY)
}

/// Whether a screen of `words` words, up for `seconds`, was really read: not
/// skimmed past, not left open, not faster than anyone reads.
pub fn counts_as_read(words: usize, seconds: f64) -> bool {
    words >= MIN_SAMPLE_WORDS
        && (MIN_SAMPLE_SECONDS..=MAX_SAMPLE_SECONDS).contains(&seconds)
        && words as f64 * 60.0 / seconds <= MAX_WPM as f64
}

/// The rate after reading `words` words in `seconds`, or None if the screen says nothing reliable.
pub fn learn(wpm: u32, words: usize, seconds: f64) -> Option<u32> {
    if words < MIN_SAMPLE_WORDS || !(MIN_SAMPLE_SECONDS..=MAX_SAMPLE_SECONDS).contains(&seconds) {
        return None;
    }
    let measured = words as f64 * 60.0 / seconds;
    Some(clamp_wpm(
        wpm as f64 + (measured - wpm as f64) * LEARN_WEIGHT,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks::Look;

    // -- test_pace.py --

    #[test]
    fn line_delay_follows_words_and_rate() {
        assert!((line_delay(10, 200) - 3.0).abs() < 1e-9);
        assert!((line_delay(10, 400) - 1.5).abs() < 1e-9);
        assert_eq!(line_delay(0, 200), MIN_DELAY); // filler-only or empty lines never stall or spin
    }

    #[test]
    fn clamp_and_step() {
        assert_eq!(clamp_wpm(10.0), MIN_WPM);
        assert_eq!(clamp_wpm(10_000.0), MAX_WPM);
        assert_eq!(clamp_wpm(f64::NAN), DEFAULT_WPM);
        assert_eq!(step_wpm(200, 1), 220);
        assert_eq!(step_wpm(220, -1), 200);
        assert_eq!(step_wpm(MAX_WPM, 1), MAX_WPM);
        assert_eq!(step_wpm(MIN_WPM, -1), MIN_WPM);
    }

    #[test]
    fn words_in_counts_only_text_segments() {
        use crate::masks::base::{Line, seg};
        let lines = [
            Line::book(
                vec![
                    seg("   1 ", Look::PLAIN),
                    seg("iki kelime", Look::PLAIN.book()),
                ],
                (0, 0),
            ),
            Line {
                segments: vec![seg("noise noise noise", Look::PLAIN)],
                is_book: false,
                pos: None,
            },
            Line::book(
                vec![
                    seg("prefix ", Look::PLAIN),
                    seg("üç kelime var", Look::PLAIN.book()),
                ],
                (0, 0),
            ),
        ];
        assert_eq!(words_in(&lines), 5);
    }

    #[test]
    fn learn_moves_toward_the_measured_rate() {
        // 100 words in 20 s = 300 wpm; a quarter of the way from 200
        assert_eq!(learn(200, 100, 20.0), Some(225));
    }

    #[test]
    fn learn_ignores_unreliable_screens() {
        assert_eq!(learn(200, 100, 1.0), None); // skimmed
        assert_eq!(learn(200, 100, 600.0), None); // walked away
        assert_eq!(learn(200, 5, 10.0), None); // too little text
    }

    #[test]
    fn learn_result_is_clamped() {
        assert_eq!(learn(MAX_WPM, 2000, 2.5), Some(MAX_WPM));
    }
}
