//! Paragraph clean-up and line layout (`text.py`).
//!
//! Widths are display cells (`unicode-width`), so wide characters never push
//! a line past the window.

use rand::{Rng, SeedableRng};
use rand_pcg::Pcg32;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Source lines rarely run longer, however wide the window.
pub const CODE_LINE_MAX: usize = 88;
const BREAK_AFTER: &[char] = &['.', ',', ';', ':', '!', '?'];
/// A ragged line ends somewhere between this share of the room and all of it.
const SHORTEST: f64 = 0.45;
/// Chance to end a long-enough line at a clause boundary.
const PUNCT_BREAK: f64 = 0.7;

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Join words hyphenated across a line break, collapse whitespace, drop empty paragraphs.
pub fn normalize_paragraphs<S: AsRef<str>>(raw: &[S]) -> Vec<String> {
    raw.iter()
        .map(|p| {
            let joined = join_hyphenated(p.as_ref());
            joined.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// Removes `-`, trailing blanks, the line break and leading blanks after a word character.
fn join_hyphenated(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-' && i > 0 && is_word_char(chars[i - 1]) {
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j], ' ' | '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\r' {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\n' {
                j += 1;
                while j < chars.len() && matches!(chars[j], ' ' | '\t') {
                    j += 1;
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The leading part of `word` that fits in `cols` cells (at least one char) and the rest.
fn take_cols(word: &str, cols: usize) -> (&str, &str) {
    let mut used = 0;
    for (i, c) in word.char_indices() {
        let w = c.width().unwrap_or(0);
        if i > 0 && used + w > cols {
            return word.split_at(i);
        }
        used += w;
    }
    (word, "")
}

/// The leading part of `s` that fits in `cells` cells (a wide char is never split).
pub fn truncate_cells(s: &str, cells: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > cells {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

/// `s` cut to `cells` cells and padded with spaces on the right to exactly that.
pub fn pad_cells(s: &str, cells: usize) -> String {
    let mut out = truncate_cells(s, cells);
    let used = out.width();
    out.push_str(&" ".repeat(cells - used));
    out
}

/// Greedy word wrap; words longer than a line are cut.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![];
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word;
        while word.width() > width {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            let (head, tail) = take_cols(word, width);
            lines.push(head.to_string());
            word = tail;
        }
        if word.is_empty() {
            continue;
        }
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.width() + 1 + word.width() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::replace(&mut cur, word.to_string()));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Words laid out like lines of source code: uneven ends, mostly at clause boundaries.
///
/// The breaks depend only on the text and the width, so the same paragraph is
/// always laid out the same way (and can be counted without being drawn).
pub fn ragged(text: &str, width: usize) -> Vec<String> {
    let room = width.clamp(1, CODE_LINE_MAX);
    let seed = crc32fast::hash(format!("{room}:{text}").as_bytes());
    let mut rng = Pcg32::seed_from_u64(seed as u64);
    let shortest = ((room as f64 * SHORTEST) as usize).max(1);
    let mut lines = vec![];
    let mut cur = String::new();
    let mut target = rng.gen_range(shortest..=room);

    for word in text.split_whitespace() {
        let mut word = word;
        while word.width() > room {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                target = rng.gen_range(shortest..=room);
            }
            let (head, tail) = take_cols(word, room);
            lines.push(head.to_string());
            word = tail;
        }
        if word.is_empty() {
            continue;
        }
        if !cur.is_empty() && cur.width() + 1 + word.width() > target {
            lines.push(std::mem::take(&mut cur));
            target = rng.gen_range(shortest..=room);
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
        let ends_clause = cur.ends_with(BREAK_AFTER);
        if cur.width() >= shortest && ends_clause && rng.r#gen::<f64>() < PUNCT_BREAK {
            lines.push(std::mem::take(&mut cur));
            target = rng.gen_range(shortest..=room);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn truncate_and_pad_count_cells() {
        assert_eq!(truncate_cells("漢字かな", 5), "漢字");
        assert_eq!(truncate_cells("abc", 5), "abc");
        assert_eq!(pad_cells("漢字かな", 5), "漢字 ");
        assert_eq!(pad_cells("ab", 4), "ab  ");
        assert_eq!(pad_cells("🙂ab", 4).width(), 4);
    }

    fn words(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }

    // -- test_text.py --

    #[test]
    fn normalize_joins_hyphenated_line_breaks() {
        assert_eq!(
            normalize_paragraphs(&["Raskol-\nnikov uyandı.", "  ", "Oda\ndar."]),
            vec!["Raskolnikov uyandı.", "Oda dar."]
        );
    }

    #[test]
    fn normalize_keeps_dashes_that_are_not_line_breaks() {
        assert_eq!(
            normalize_paragraphs(&["iyi - kötü", "-\nbaşta"]),
            vec!["iyi - kötü", "- başta"]
        );
    }

    #[test]
    fn wrap_respects_width_and_keeps_all_words() {
        let text = "Oda dar ve basıktı, tavanı alçaktı.";
        let lines = wrap(text, 12);
        assert!(lines.iter().all(|l| l.width() <= 12));
        assert_eq!(words(&lines.join(" ")), words(text));
    }

    #[test]
    fn wrap_hard_splits_overlong_word() {
        assert_eq!(wrap(&"a".repeat(30), 10), vec!["a".repeat(10); 3]);
    }

    #[test]
    fn wrap_zero_width_does_not_hang() {
        assert_eq!(wrap("ab cd", 0).concat(), "abcd");
    }

    #[test]
    fn wrap_counts_wide_characters_as_two_cells() {
        assert_eq!(wrap("漢字漢字", 4), vec!["漢字", "漢字"]);
    }

    const PROSE: &str = "Sabah erkenden uyandı, pencereyi açtı ve dışarı baktı. Sokaklar sessizdi; \
        yalnızca uzaktan bir vapur düdüğü geliyordu. Çayını demledi, masaya oturdu, \
        dünkü mektubu yeniden okudu. Cevap yazmak istiyordu ama ne diyeceğini bilmiyordu. ";

    fn prose() -> String {
        PROSE.repeat(4)
    }

    #[test]
    fn ragged_keeps_every_word_in_order_and_fits() {
        let text = prose();
        for width in [20, 60, 120, 200] {
            let lines = ragged(&text, width);
            assert_eq!(words(&lines.join(" ")), words(&text));
            assert!(
                lines
                    .iter()
                    .all(|l| !l.is_empty() && l.width() <= width.min(CODE_LINE_MAX))
            );
        }
    }

    #[test]
    fn ragged_line_ends_vary_like_code() {
        let lines = ragged(&prose(), 100);
        let body = &lines[..lines.len() - 1]; // the last line is short anyway
        let lengths: HashSet<usize> = body.iter().map(|l| l.width()).collect();
        assert!(lengths.len() > body.len() / 2);
        let (min, max) = (
            *lengths.iter().min().unwrap(),
            *lengths.iter().max().unwrap(),
        );
        assert!((min as f64) < 0.7 * max as f64);
    }

    #[test]
    fn ragged_prefers_breaking_after_punctuation() {
        let lines = ragged(&prose(), 100);
        let at_clause = lines.iter().filter(|l| l.ends_with(BREAK_AFTER)).count();
        assert!(at_clause > lines.len() / 2);
    }

    #[test]
    fn ragged_is_stable_for_the_same_text_and_width() {
        assert_eq!(ragged(&prose(), 80), ragged(&prose(), 80));
    }

    #[test]
    fn ragged_hard_splits_overlong_words_and_tiny_widths() {
        assert_eq!(
            ragged(&"a".repeat(25), 10),
            vec!["a".repeat(10), "a".repeat(10), "a".repeat(5)]
        );
        assert!(!ragged(&prose(), 1).is_empty()); // no hang
    }
}
