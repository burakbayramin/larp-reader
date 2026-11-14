//! A live-looking agent session: animated status, typewriter, input box.

use super::*;

// -- a live-looking agent session --

fn agent() -> Config {
    Config {
        mask: "agent".into(),
        ..cfg()
    }
}

#[test]
fn agent_status_spins_and_counts_seconds() {
    let mut h = sample_with(10, agent(), 80, 24);
    let first = h.status();
    assert!(first.starts_with("· "), "{first}");
    assert!(first.contains("(0s · 1/10 · "), "{first}");
    h.advance(ANIMATION_FRAME);
    let next = h.status();
    assert_ne!(next, first);
    assert!(next.starts_with("✢ "), "{next}");
    h.advance(Duration::from_secs(12));
    assert!(h.status().contains("(12s · "), "{}", h.status());
    assert!(h.screen().contains("(12s · "));
    for w in ["book", "reader", "okuyucu", "esc to"] {
        assert!(!h.status().to_lowercase().contains(w), "{w}");
    }
    h.press(&["m"]); // another view: a still label again
    assert!(h.status().contains(" 1/10"));
    assert!(!h.app.animating());
}

#[test]
fn deadline_is_near_only_while_the_label_animates() {
    let mut h = sample_with(10, agent(), 80, 24);
    assert!(h.app.animating());
    assert!(h.app.next_deadline() <= h.now + ANIMATION_FRAME);
    h.press(&["`"]); // hidden: its own timer only
    assert!(!h.app.animating());
    assert!(h.app.next_deadline() > h.now + ANIMATION_FRAME);
    h.press(&["`", "+"]); // a flash replaces the label for a while
    assert!(!h.app.animating());
    h.advance(FLASH);
    assert!(h.app.animating());
    let mut code = Harness::sample(80, 24);
    assert!(!code.app.animating());
    assert!(code.app.next_deadline() >= code.now + Duration::from_secs(1));
    code.advance(Duration::from_secs(1));
    assert!(code.app.next_deadline() >= code.now + Duration::from_secs(1));
}

/// The newest text line of the stream, as drawn on screen.
fn newest_drawn(h: &Harness) -> (Line, String) {
    let line = h.app.feed.history_lines().last().unwrap().clone();
    let text = h.app.feed.text();
    (line, text.lines.last().unwrap().to_string())
}

#[test]
fn typewriter_reveals_a_streamed_line_then_completes() {
    let mut h = sample_with(10, agent(), 80, 24);
    h.press(&["a"]);
    h.app.tick();
    let (line, drawn) = newest_drawn(&h);
    assert!(line.is_book); // the history has the whole line at once
    assert_eq!(h.app.feed.reveal(), Some(0));
    assert!(h.app.next_deadline() <= h.now + TYPING_FRAME);
    let full = line.plain();
    assert_ne!(drawn, full);
    let mut seen = vec![0];
    for _ in 0..200 {
        h.advance(TYPING_FRAME);
        match h.app.feed.reveal() {
            Some(n) => seen.push(n),
            None => break,
        }
    }
    assert_eq!(h.app.feed.reveal(), None, "finished");
    assert!(seen.len() > 3, "{seen:?}"); // several steps
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
    assert_eq!(newest_drawn(&h).1, full);
    // and well before the next line is due
    assert!(h.app.timer_due().unwrap() > h.now);
    assert!(h.app.next_deadline() > h.now + TYPING_FRAME);
}

#[test]
fn typewriter_finishes_at_once_on_keys_hiding_and_resizing() {
    let mut h = sample_with(10, agent(), 80, 24);
    h.press(&["a"]);
    h.app.tick();
    h.advance(TYPING_FRAME);
    assert!(h.app.feed.reveal().is_some());
    h.press(&["space"]); // paging is idle in auto mode, but the line shows whole
    assert_eq!(h.app.feed.reveal(), None);
    assert_eq!(newest_drawn(&h).1, newest_drawn(&h).0.plain());
    h.app.tick();
    h.press(&["`"]);
    assert_eq!(h.app.feed.reveal(), None);
    assert!(!h.app.feed.history_lines().iter().any(|l| l.is_book));
    for _ in 0..5 {
        h.app.tick(); // the hidden stream never types out
        h.advance(TYPING_FRAME);
        assert_eq!(h.app.feed.reveal(), None);
    }
    h.press(&["`"]);
    h.app.tick();
    assert!(h.app.feed.reveal().is_some());
    h.app.resize(100, 30);
    assert_eq!(h.app.feed.reveal(), None);
    h.app.tick();
    h.press(&["a"]); // leaving auto mode
    assert_eq!(h.app.feed.reveal(), None);
}

#[test]
fn other_views_stream_whole_lines() {
    let mut h = sample_with(10, cfg(), 80, 24);
    h.press(&["a"]);
    h.app.tick();
    assert_eq!(h.app.feed.reveal(), None);
}

#[test]
fn agent_view_draws_an_input_box_above_the_status() {
    let h = sample_with(10, agent(), 80, 24);
    let rows: Vec<String> = h.screen().lines().map(String::from).collect();
    assert!(
        rows[20].starts_with('╭') && rows[20].ends_with('╮'),
        "{}",
        rows[20]
    );
    assert!(rows[21].starts_with("│ > "), "{}", rows[21]);
    assert!(rows[22].starts_with('╰'));
    assert!(rows[23].contains("1/10"));
    assert_eq!(h.visible().len(), 20);
    for w in ["book", "reader", "okuyucu", "sample"] {
        assert!(!h.screen().to_lowercase().contains(w), "{w}");
    }
    // other views, narrow or short windows and the hidden screen have none
    let code = Harness::sample(80, 24);
    assert!(!code.screen().contains('╭'));
    assert_eq!(code.visible().len(), 23);
    for (cols, rows) in [(39, 24), (80, 9)] {
        let small = sample_with(10, agent(), cols, rows);
        assert!(!small.screen().contains('╭'), "{cols}x{rows}");
    }
    let mut h = h;
    h.press(&["`"]);
    assert!(!h.screen().contains('╭'));
    assert_eq!(h.visible().len(), 23); // the hidden stream fills the screen
    h.press(&["`"]);
    assert!(h.screen().contains('╭'));
    assert_eq!(h.visible().len(), 20);
}

#[test]
fn paging_with_the_input_box_shows_every_line_once() {
    let mut h = sample_with(30, agent(), 80, 24);
    let header = get_mask("agent", "python")
        .unwrap()
        .header(&mut seeded("x"), 80);
    let stream = h.session().window(100_000);
    let first = h.visible();
    assert_eq!(first[..header.len()], header[..]);
    let mut got: Vec<Line> = first[header.len()..].to_vec();
    for _ in 0..200 {
        if !h.app.more_below {
            break;
        }
        h.press(&["space"]);
        let page = h.visible();
        assert!(page.len() <= 20);
        got.extend(page);
    }
    assert!(!h.app.more_below);
    assert_eq!(got.len(), stream.len());
    assert_eq!(got, stream);
    h.press(&["u"]); // and back again a screen
    assert!(h.visible().len() <= 20);
}
