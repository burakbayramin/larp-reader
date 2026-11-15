//! The hidden screen: its stream and the fake shell.

use super::*;
use crate::pace::learn;

#[test]
fn panic_removes_every_word_of_the_text() {
    let mut h = Harness::sample(80, 24);
    let words = text_words(&mut h);
    assert!(!leaked(&h, &words).is_empty()); // the text WAS visible
    h.press(&["a"]);
    for _ in 0..5 {
        h.app.tick();
    }
    h.press(&["`"]);
    assert!(h.app.panic);
    assert!(h.app.feed.history_lines().len() >= 23); // a screenful before the first tick
    for _ in 0..30 {
        h.app.tick();
    }
    assert!(!shown(&h).trim().is_empty());
    assert_eq!(leaked(&h, &words), Vec::<String>::new());
    assert!(!h.app.feed.history_lines().iter().any(|l| l.is_book));
}

#[test]
fn panic_toggle_returns_to_the_same_place_and_mode() {
    let mut h = Harness::sample(80, 24);
    h.press(&["down", "down"]);
    let pos = h.session().pos();
    h.press(&["`"]);
    for _ in 0..5 {
        h.app.tick();
    }
    h.press(&["space", "down", "a"]); // manual keys do nothing while hidden
    assert_eq!(h.session().pos(), pos);
    assert!(!h.app.auto);
    h.press(&["`"]);
    assert!(!h.app.panic && !h.app.auto);
    assert!(h.visible().iter().any(|l| l.pos == Some(pos)));
    h.press(&["a", "`", "`"]);
    assert!(h.app.auto && !h.app.panic);
    assert_eq!(h.session().pos(), pos);
}

#[test]
fn hidden_stream_flows_on_a_timer() {
    let mut h = Harness::sample(80, 24);
    h.press(&["`"]);
    let n = h.app.feed.history_lines().len();
    let due = h.app.timer_due().unwrap();
    h.now = due;
    h.app.on_time(due);
    assert_eq!(h.app.feed.history_lines().len(), n + 1);
}

#[test]
fn time_spent_hidden_is_not_reading_time() {
    let mut h = paced(200);
    let words = h.app.pacer.view_words;
    h.press(&["`"]);
    h.now += Duration::from_secs(120); // someone walked by
    h.press(&["`"]);
    h.now += Duration::from_secs(15);
    h.press(&["space"]);
    assert_eq!(h.app.config.wpm, learn(200, words, 15.0).unwrap());
}

fn shell_harness() -> Harness {
    sample_with(
        10,
        Config {
            hide: "shell".into(),
            ..cfg()
        },
        80,
        24,
    )
}

#[test]
fn shell_hide_shows_a_prompt_that_takes_commands() {
    let mut h = shell_harness();
    let words = text_words(&mut h);
    h.press(&["`"]);
    assert!(h.app.panic && h.app.shell.is_some());
    assert!(
        h.app
            .timer_due()
            .is_none_or(|t| t > h.now + Duration::from_secs(5))
    ); // a shell waits, it does not stream
    let screen = h.screen();
    assert!(
        screen
            .lines()
            .last()
            .unwrap()
            .trim_end()
            .ends_with(['$', '%', '>']),
        "{screen}"
    );
    type_text(&mut h, "ls");
    h.press(&["enter"]);
    let lines: Vec<String> = h.app.feed.history_lines().iter().map(Line::plain).collect();
    assert!(lines.iter().any(|l| l.ends_with(" ls")));
    assert!(lines.join(" ").contains("src"));
    assert_eq!(
        leaked(&h, &words)
            .into_iter()
            .filter(|w| w != "number")
            .count(),
        0
    );
    type_text(&mut h, "jk q");
    assert_eq!((h.session().pos(), h.session().line_offset), ((0, 0), 0));
    assert!(!h.app.should_quit()); // q is just a letter at the prompt
    h.press(&["`"]);
    assert!(!h.app.panic && h.app.shell.is_none());
}

#[test]
fn a_long_prompt_scrolls_so_the_typed_text_stays_in_view() {
    assert_eq!(
        tail_fitting("runner@a-very-long-host-name:~/work$ ls", 12),
        "e:~/work$ ls"
    );
    assert_eq!(tail_fitting("short$ ", 40), "short$ ");
    let mut h = shell_harness();
    h.app.prompt = format!("runner@{}:~/work/larpr$ ", "x".repeat(100));
    h.press(&["`"]);
    type_text(&mut h, "git status");
    assert!(
        h.screen()
            .lines()
            .last()
            .unwrap()
            .trim_end()
            .ends_with("$ git status")
    );
}

#[test]
fn shell_clear_and_unknown_commands() {
    let mut h = shell_harness();
    h.press(&["`"]);
    type_text(&mut h, "clear");
    h.press(&["enter"]);
    assert!(h.app.feed.history_lines().is_empty());
    type_text(&mut h, "frobnicate");
    h.press(&["enter"]);
    let text = shown(&h);
    assert!(
        text.contains("frobnicate")
            && (text.contains("not found") || text.contains("not recognized"))
    );
}
