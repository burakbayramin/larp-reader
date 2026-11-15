//! Reading statistics: what counts as read, the day it counts on, and the file.

use super::*;
use crate::state::load_stats;

const DAY: i64 = 20_772; // 2026-11-15

fn counted() -> Harness {
    let mut h = paced(200);
    h.app.day_override = Some(DAY);
    h
}

#[test]
fn paging_after_a_real_look_counts_words_and_time() {
    let mut h = counted();
    let words = h.app.pacer.view_words as u64;
    assert!(words >= 15);
    let id = h.session().book.id.clone();
    h.now += Duration::from_secs(20);
    h.press(&["space"]);
    let day = h.app.stats.day(DAY);
    assert_eq!(day.words, words);
    assert!((day.seconds - 20.0).abs() < 0.01);
    assert_eq!(h.app.stats.book(&id).words, words);
}

#[test]
fn skipping_screens_quickly_counts_nothing() {
    let mut h = counted();
    for _ in 0..4 {
        h.now += Duration::from_millis(900);
        h.press(&["space"]);
    }
    assert!(h.app.stats.is_empty());
}

#[test]
fn a_screen_left_open_for_ages_counts_nothing() {
    let mut h = counted();
    h.now += Duration::from_secs(900); // away from the desk
    h.press(&["space"]);
    assert!(h.app.stats.is_empty());
}

#[test]
fn time_hidden_is_not_counted() {
    let mut h = counted();
    h.now += Duration::from_secs(1);
    h.press(&["`"]); // hidden after a glance: too short to count
    h.now += Duration::from_secs(120);
    h.press(&["`"]);
    h.now += Duration::from_secs(15);
    h.press(&["space"]);
    let day = h.app.stats.day(DAY);
    assert!((day.seconds - 15.0).abs() < 0.01, "{day:?}");
}

#[test]
fn reading_while_hidden_adds_nothing() {
    let mut h = counted();
    h.press(&["`"]);
    h.now += Duration::from_secs(30);
    h.press(&["space", "space"]); // keys do nothing hidden
    assert!(h.app.stats.is_empty());
}

#[test]
fn each_day_keeps_its_own_totals() {
    let mut h = counted();
    h.now += Duration::from_secs(20);
    h.press(&["space"]);
    h.app.day_override = Some(DAY + 1);
    h.now += Duration::from_secs(20);
    h.press(&["space"]);
    let (a, b) = (h.app.stats.day(DAY), h.app.stats.day(DAY + 1));
    assert!(a.words > 0 && b.words > 0);
    assert_eq!(h.app.stats.last(DAY + 1, 2).words, a.words + b.words);
    assert_eq!(h.app.stats.streak(DAY + 1), 2);
}

#[test]
fn auto_scroll_counts_the_lines_it_shows() {
    let mut h = counted();
    h.press(&["a"]);
    for _ in 0..6 {
        let due = h.app.timer_due().expect("streaming");
        h.now = due;
        h.app.on_time(h.now);
    }
    assert!(h.app.stats.day(DAY).words > 0);
}

#[test]
fn quitting_saves_the_screen_being_read_without_naming_the_text() {
    let mut h = counted();
    h.now += Duration::from_secs(20);
    h.press(&["q"]);
    let saved = load_stats(&h.state_dir());
    assert!(saved.day(DAY).words > 0);
    let raw = std::fs::read_to_string(h.state_dir().join("stats.json")).unwrap();
    assert!(!raw.contains("sample") && !raw.contains("Paragraph"));
}

#[test]
fn autosave_writes_the_statistics_too() {
    let mut h = counted();
    h.now += Duration::from_secs(20);
    h.press(&["space"]);
    h.advance(AUTOSAVE);
    assert!(load_stats(&h.state_dir()).day(DAY).words > 0);
}

#[test]
fn a_corrupt_file_starts_empty_and_is_kept_as_a_backup() {
    let d = tempfile::tempdir().unwrap();
    let p = make_txt(d.path(), 10);
    let st = d.path().join("st");
    std::fs::create_dir_all(&st).unwrap();
    std::fs::write(st.join("stats.json"), "][").unwrap();
    let h = Harness::new(Some(p), cfg(), d, 80, 24);
    assert!(h.app.stats.is_empty());
    assert!(st.join("stats.json.bak").exists());
}
