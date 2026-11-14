//! Auto scroll and the reading rate it learns from paging.

use super::*;

fn noisy() -> Config {
    Config { noise: 3, ..cfg() }
}

fn book_count(h: &Harness) -> usize {
    h.app
        .feed
        .history_lines()
        .iter()
        .filter(|l| l.is_book)
        .count()
}

#[test]
fn auto_tick_appends_one_book_line_plus_filler() {
    let mut h = sample_with(10, noisy(), 80, 24);
    h.press(&["a"]);
    assert!(h.app.auto);
    for _ in 0..6 {
        let (books, all) = (book_count(&h), h.app.feed.history_lines().len());
        h.app.tick();
        assert_eq!(book_count(&h), books + 1);
        assert!(h.app.feed.history_lines().len() > all);
    }
}

#[test]
fn auto_is_bottom_anchored_and_capped() {
    let mut h = sample_with(400, noisy(), 80, 24);
    h.press(&["a"]);
    for _ in 0..40 {
        h.app.tick();
    }
    let hist = h.app.feed.history_lines().to_vec();
    assert_eq!(h.visible(), hist[hist.len() - h.visible().len()..].to_vec());
    for _ in 0..1500 {
        h.app.tick();
    }
    assert_eq!(h.app.feed.history_lines().len(), crate::ui::feed::MAX_LINES);
}

#[test]
fn wpm_is_clamped_stepped_shown_and_saved() {
    let mut h = sample_with(10, Config { wpm: 200, ..cfg() }, 80, 24);
    h.press(&["+"]);
    assert_eq!(h.app.config.wpm, 220);
    assert!(h.status().contains("220/min"));
    h.press(&["-"]);
    assert_eq!(h.app.config.wpm, 200);
    assert_eq!(crate::state::load_config(&h.state_dir()).wpm, 200);
    h.press(&["+"; 60]);
    assert_eq!(h.app.config.wpm, 1500);
    h.press(&["-"; 80]);
    assert_eq!(h.app.config.wpm, 60);
    h.advance(FLASH + Duration::from_millis(1));
    assert!(!h.status().contains("/min"));
}

#[test]
fn config_auto_and_wpm_apply_at_startup() {
    let h = sample_with(
        10,
        Config {
            auto: true,
            wpm: 400,
            ..cfg()
        },
        80,
        24,
    );
    assert!(h.app.auto);
    assert_eq!(h.app.config.wpm, 400);
    assert!(h.app.timer_due().is_some());
}

#[test]
fn auto_reaches_the_end_stops_and_manual_keys_work_again() {
    let mut h = sample_with(3, cfg(), 80, 24);
    h.press(&["a"]);
    for _ in 0..200 {
        h.app.tick();
    }
    assert_eq!(h.session().progress(), (3, 3));
    assert_eq!(h.session().pos(), (0, 2));
    assert!(!h.app.auto && h.app.timer_due().is_none());
    let n = h.app.feed.history_lines().len();
    h.app.tick();
    assert_eq!(h.app.feed.history_lines().len(), n);
    let top = h.visible()[0].clone();
    h.press(&["k"]);
    assert_ne!(h.visible()[0], top);
}

#[test]
fn auto_waits_by_the_words_on_screen_then_by_each_line() {
    let mut h = paced(300);
    h.press(&["a"]);
    let first = words_in(&h.session().paragraph_lines((0, 0)));
    let due = h.app.timer_due().unwrap();
    assert_eq!(due, h.now + Duration::from_secs_f64(line_delay(first, 300)));
    let before = h.app.feed.history_lines().len();
    h.now = due;
    h.app.on_time(due);
    let new = h.app.feed.history_lines()[before..].to_vec();
    assert!(words_in(&new) > 0);
    assert_eq!(
        h.app.timer_due().unwrap(),
        due + Duration::from_secs_f64(line_delay(words_in(&new), 300))
    );
}

#[test]
fn paging_learns_the_reading_rate() {
    let mut h = paced(200);
    let words = h.app.view_words;
    assert!(words > 20);
    h.now += Duration::from_secs(20);
    h.press(&["space"]);
    let want = learn(200, words, 20.0).unwrap();
    assert!(want != 200 && h.app.config.wpm == want);
    assert_eq!(crate::state::load_config(&h.state_dir()).wpm, want);
}

#[test]
fn quick_paging_and_line_scrolling_do_not_learn() {
    let mut h = paced(200);
    h.now += Duration::from_millis(500);
    h.press(&["space"]); // skimmed
    h.now += Duration::from_secs(30);
    h.press(&["j", "j"]);
    assert_eq!(h.app.config.wpm, 200);
}

#[test]
fn line_scrolling_keeps_the_clock_and_counts_the_new_lines() {
    // a screen read for 20 s, a line scrolled, then paged 3 s later: 23 s of reading
    let mut h = paced(200);
    let before = h.app.view_words;
    h.now += Duration::from_secs(20);
    h.press(&["j", "j"]);
    h.now += Duration::from_secs(3);
    let read = h.app.view_words;
    assert!(read >= before);
    h.press(&["space"]);
    assert_eq!(h.app.config.wpm, learn(200, read, 23.0).unwrap());
}
