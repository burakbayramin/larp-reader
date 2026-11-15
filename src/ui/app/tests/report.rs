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

fn status_text(h: &Harness) -> String {
    h.app
        .status_segments()
        .into_iter()
        .map(|(t, _)| t)
        .collect()
}

#[test]
fn the_status_line_carries_todays_words_when_there_is_room() {
    let h = read_some("agent");
    let today = h.app.stats.day(DAY).words;
    let status = status_text(&h);
    assert!(
        status.ends_with(&format!("today {} words", compact(today))),
        "{status}"
    );
    assert!(status.contains("accept edits")); // the view's own pieces stay
}

#[test]
fn the_status_line_stays_plain_before_any_reading_and_when_narrow() {
    let mut h = sample_with(
        10,
        Config {
            mask: "agent".into(),
            ..cfg()
        },
        100,
        30,
    );
    h.app.day_override = Some(DAY);
    assert!(!status_text(&h).contains("today"));
    let mut h = read_some("agent");
    h.app.resize(50, 30);
    let status = status_text(&h);
    assert!(
        !status.contains("today") && status.chars().count() <= 50,
        "{status}"
    );
}
