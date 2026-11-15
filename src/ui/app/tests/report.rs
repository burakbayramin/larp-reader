//! The statistics screen: the key, what it shows, and that it stays in disguise.

use super::*;
use crate::masks::MASK_NAMES;
use crate::ui::report::compact;

const DAY: i64 = 20_772; // 2026-11-15

/// A text with some pages read on `DAY`, and the report asked for.
fn read_some(mask: &str) -> Harness {
    let mut h = sample_with(
        30,
        Config {
            mask: mask.into(),
            wpm: 200,
            ..cfg()
        },
        100,
        30,
    );
    h.app.day_override = Some(DAY);
    for _ in 0..3 {
        h.now += Duration::from_secs(20);
        h.press(&["space"]);
    }
    h
}

#[test]
fn the_key_opens_it_and_any_key_closes_it() {
    let mut h = read_some("agent");
    let before = h.session().pos();
    h.press(&["S"]);
    assert!(h.app.report.is_some());
    h.press(&["space"]); // closes, and does not page on
    assert!(h.app.report.is_none());
    assert_eq!(h.session().pos(), before);
}

#[test]
fn it_shows_the_recorded_numbers() {
    let mut h = read_some("agent");
    let today = h.app.stats.day(DAY);
    assert!(today.words > 0);
    h.press(&["S"]);
    let screen = h.screen();
    assert!(screen.contains("today"), "{screen}");
    assert!(
        screen.contains(&format!("{} words", compact(today.words))),
        "{screen}"
    );
    assert!(screen.contains("1 days"), "{screen}"); // the streak
    assert!(
        screen.contains("ETA") && screen.contains("at current pace"),
        "{screen}"
    );
    assert!(screen.chars().any(|c| ('▁'..='█').contains(&c)), "{screen}");
}

#[test]
fn panic_hides_it_at_once() {
    let mut h = read_some("agent");
    h.press(&["S", "`"]);
    assert!(h.app.report.is_none() && h.app.panic);
    assert!(!h.screen().contains("pages"));
}

#[test]
fn no_mask_gives_the_game_away() {
    for mask in MASK_NAMES {
        let mut h = read_some(mask);
        h.press(&["S"]);
        let screen = h.screen().to_lowercase();
        assert!(screen.contains("pages"), "{mask}:\n{screen}");
        for word in ["book", "reader", "okuyucu", "sample", "paragraph", "lorem"] {
            assert!(!screen.contains(word), "{mask} shows {word}:\n{screen}");
        }
    }
}

#[test]
fn the_report_is_not_reading_time() {
    let mut h = read_some("log");
    let words = h.app.stats.day(DAY).words;
    h.press(&["S"]);
    h.now += Duration::from_secs(60); // staring at the numbers
    h.press(&["x"]);
    h.now += Duration::from_secs(1);
    h.press(&["space"]); // a screen up for one second: skimmed
    assert_eq!(h.app.stats.day(DAY).words, words);
}

#[test]
fn it_waits_for_nothing_while_hidden() {
    let mut h = read_some("code");
    h.press(&["`"]);
    h.press(&["S"]);
    assert!(h.app.report.is_none());
}
