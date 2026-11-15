//! A live-looking agent session: animated status, typewriter, input box.

use super::*;
use crate::ui::timer::{ANIMATION_FRAME, TYPING_FRAME};

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
    let text = h.app.feed.text(&h.app.theme);
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
        assert!(!small.screen().contains("│ > "), "{cols}x{rows}");
    }
    let mut h = h;
    h.press(&["`"]);
    assert!(!h.screen().contains("│ > ") && !h.screen().contains('╭'));
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

// -- the welcome figure --

#[test]
fn the_welcome_figure_poses_flourishes_and_marches_while_text_streams() {
    use crate::masks::agent::{SHOW_EVERY, SHOW_FOR, SPRITE, STEP_FOR};
    let mut h = sample_with(10, agent(), 80, 40);
    let screen = h.screen();
    for row in SPRITE {
        assert!(screen.contains(row), "{row:?} missing:\n{screen}");
    }
    h.advance(SHOW_EVERY - SHOW_FOR + ANIMATION_FRAME / 2);
    let flourish = h.screen();
    assert!(
        !flourish.contains(SPRITE[0]) && !flourish.contains(SPRITE[1]),
        "{flourish}"
    );
    assert!(flourish.contains(SPRITE[2]) && flourish.contains(SPRITE[3]));
    h.advance(SHOW_FOR);
    for row in SPRITE {
        assert!(h.screen().contains(row), "pose back: {row:?}");
    }

    let mut a = sample_with(
        10,
        Config {
            auto: true,
            ..agent()
        },
        80,
        60,
    );
    a.advance(STEP_FOR + ANIMATION_FRAME / 2);
    let marching = a.screen();
    assert!(marching.contains(SPRITE[1]), "{marching}");
    assert!(!marching.contains(SPRITE[3]), "{marching}");
    a.press(&["`"]); // hidden: no figure at all
    for row in SPRITE {
        assert!(!a.screen().contains(row));
    }
}

#[test]
fn the_header_itself_never_changes_with_the_figure() {
    let mut h = sample_with(10, agent(), 80, 40);
    let before = h.app.feed.history_lines().to_vec();
    h.advance(Duration::from_secs(4));
    assert_eq!(h.app.feed.history_lines().to_vec(), before);
}

#[test]
fn the_welcome_box_greets_a_resumed_session_too() {
    use crate::masks::agent::SPRITE;
    let mut h = sample_with(30, agent(), 80, 40);
    h.press(&["space", "space", "space"]);
    let pos = h.session().pos();
    assert_ne!(pos, (0, 0));
    assert!(
        !h.screen().contains(SPRITE[0]),
        "only where the session started"
    );
    h.press(&["q"]);
    let mut again = Harness {
        app: App::new(
            Some(h.dir.join("sample.txt")),
            agent(),
            Theme::default(),
            h.state_dir(),
            80,
            40,
            h.now,
        ),
        now: h.now,
        _dir: tempfile::tempdir().unwrap(),
        dir: h.dir.clone(),
    };
    assert_eq!(again.session().pos(), pos);
    for row in SPRITE {
        assert!(again.screen().contains(row), "{row:?}:\n{}", again.screen());
    }
    again.press(&["space"]);
    assert!(!again.screen().contains(SPRITE[0]), "gone once you move on");
}

// -- typing into the input box --

/// The box's middle row (`│ > … │`) and where the terminal cursor sits.
fn box_row(h: &Harness) -> (String, Option<(u16, u16)>) {
    let mut t = Terminal::new(TestBackend::new(h.app.cols as u16, h.app.rows as u16)).unwrap();
    t.draw(|f| h.app.draw(f)).unwrap();
    // a fresh terminal's cursor sits at the corner until a draw places it
    let p = t.get_cursor_position().unwrap();
    let cursor = Some((p.x, p.y)).filter(|&c| c != (0, 0));
    let row = h.screen().lines().nth(h.app.rows - 3).unwrap().to_string();
    (row, cursor)
}

#[test]
fn i_focuses_the_box_and_typing_shows_with_a_cursor() {
    let mut h = sample_with(10, agent(), 80, 24);
    let before = h.visible();
    h.press(&["i"]);
    let (row, cursor) = box_row(&h);
    assert!(row.starts_with("│ > ") && !row.contains("Try"), "{row}");
    assert_eq!(cursor, Some((4, 21)));
    type_text(&mut h, "add retries jk q");
    h.press(&["backspace", "backspace"]);
    let (row, cursor) = box_row(&h);
    assert_eq!(row.trim_end_matches(['│', ' ']), "│ > add retries jk");
    assert_eq!(cursor, Some((4 + 14, 21)));
    assert_eq!(h.visible(), before, "j, k and space typed, nothing moved");
    assert!(!h.app.should_quit(), "q typed, not quitting");
    // a long request scrolls sideways and keeps the cursor inside the box
    type_text(&mut h, &"x".repeat(200));
    let (row, cursor) = box_row(&h);
    assert!(row.ends_with("x │"), "{row}");
    assert!(cursor.unwrap().0 < 79);
    for w in ["book", "reader", "okuyucu", "sample"] {
        assert!(!h.screen().to_lowercase().contains(w), "{w}");
    }
}

#[test]
fn esc_cancels_typing_and_leaves_the_text_where_it_was() {
    let mut h = sample_with(30, agent(), 80, 24);
    h.press(&["space"]);
    let (pos, offset, before) = (h.session().pos(), h.session().line_offset, h.visible());
    h.press(&["i"]);
    type_text(&mut h, "never mind");
    h.press(&["escape"]);
    assert!(h.app.ask.is_none());
    assert!(!h.app.panel, "esc closed the box, not opened the panel");
    let (row, cursor) = box_row(&h);
    assert!(row.contains("Try"), "{row}");
    assert_eq!(cursor, None);
    assert!(!h.screen().contains("never mind"));
    assert_eq!((h.session().pos(), h.session().line_offset), (pos, offset));
    assert_eq!(h.visible(), before);
    h.press(&["space"]); // reading keys work again
    assert_ne!(h.session().pos(), pos);
}

#[test]
fn typing_pauses_auto_scroll_until_the_box_closes() {
    let mut h = sample_with(
        30,
        Config {
            auto: true,
            ..agent()
        },
        80,
        24,
    );
    assert!(h.app.timer_due().is_some());
    h.press(&["i"]);
    assert!(h.app.timer_due().is_none(), "auto waits while typing");
    let lines = h.app.feed.history_lines().len();
    h.advance(Duration::from_secs(30));
    assert_eq!(h.app.feed.history_lines().len(), lines);
    h.press(&["escape"]);
    assert!(h.app.auto);
    assert!(h.app.timer_due().is_some(), "and goes on after");
}

#[test]
fn hiding_while_typing_drops_the_box_and_what_was_typed() {
    let mut h = sample_with(10, agent(), 80, 24);
    let words = text_words(&mut h);
    h.press(&["i"]);
    type_text(&mut h, "zebra quokka");
    h.press(&["`"]); // hides at once, and types no backtick
    assert!(h.app.panic && h.app.ask.is_none());
    let screen = h.screen();
    assert!(
        !screen.contains("zebra") && !screen.contains('`'),
        "{screen}"
    );
    assert!(!screen.contains('╭'));
    assert_eq!(leaked(&h, &words), Vec::<String>::new());
    h.press(&["i"]); // no box while hidden
    assert!(h.app.ask.is_none());
    h.press(&["`"]);
    assert!(!h.screen().contains("zebra"));
    assert!(h.screen().contains("Try"));
}

#[test]
fn the_box_key_is_inert_in_other_views_and_small_windows() {
    let mut h = sample_with(30, cfg(), 80, 24);
    let before = h.visible();
    h.press(&["i"]);
    assert!(h.app.ask.is_none());
    h.press(&["j"]);
    assert_ne!(h.visible(), before, "j still scrolls");
    let mut small = sample_with(10, agent(), 80, 9);
    small.press(&["i"]);
    assert!(small.app.ask.is_none());
    let mut h = sample_with(10, agent(), 80, 24);
    h.press(&["i"]);
    h.app.resize(80, 9); // the box goes away, and typing with it
    assert!(h.app.ask.is_none());
}

// -- the fake reply --

const REQUEST: &str = "add retries to the upload client";

/// Type `request` into the box and send it.
fn ask(h: &mut Harness, request: &str) {
    h.press(&["i"]);
    type_text(h, request);
    h.press(&["enter"]);
}

/// Every line of the reply to `request`: played until just before it ends.
fn whole_reply(h: &mut Harness, request: &str) -> Vec<Line> {
    let before = h.app.feed.history_lines().len();
    ask(h, request);
    let left = h.app.reply.as_ref().unwrap().length();
    h.advance(left - crate::ui::reply::HOLD / 2);
    assert!(h.app.reply.is_some());
    let lines = &h.app.feed.history_lines()[before..];
    // a gap above the request when the screen did not end in one
    let gap = lines
        .iter()
        .take_while(|l| l.plain().trim().is_empty())
        .count();
    assert!(gap <= 1);
    lines[gap..].to_vec()
}

/// Let the reply play out, a tenth of a second at a time, for up to 15s.
fn play_out(h: &mut Harness) {
    for _ in 0..150 {
        if h.app.reply.is_none() {
            return;
        }
        h.advance(Duration::from_millis(100));
    }
}

#[test]
fn enter_echoes_the_request_and_a_reply_streams_in() {
    let mut h = sample_with(30, agent(), 80, 24);
    let words = text_words(&mut h);
    let before = h.visible();
    ask(&mut h, REQUEST);
    assert!(h.app.ask.is_none() && h.app.reply.is_some());
    let screen = h.screen();
    assert!(screen.contains(&format!("> {REQUEST}")), "{screen}");
    let (row, cursor) = box_row(&h);
    assert!(
        !row.contains("Try") && cursor.is_none(),
        "an empty box: {row}"
    );
    h.advance(Duration::from_millis(400));
    assert!(h.screen().contains("✻ Thinking…"));
    let shown = h.app.feed.history_lines().len();
    h.advance(Duration::from_secs(3));
    assert!(
        h.app.feed.history_lines().len() > shown,
        "it goes on working"
    );
    // done within a few seconds, and the text is back exactly as it was
    play_out(&mut h);
    assert!(h.app.reply.is_none(), "over within 15s");
    assert_eq!(h.visible(), before);
    assert!(!h.screen().contains(REQUEST));
    assert!(h.screen().contains("Try"));
    let mut fresh = sample_with(30, agent(), 80, 24);
    let reply = whole_reply(&mut fresh, REQUEST);
    assert!(reply.iter().all(|l| !l.is_book && l.pos.is_none()));
    let text = reply.iter().map(Line::plain).collect::<Vec<_>>().join("\n");
    assert!(fresh.screen().contains("⏺ "), "{}", fresh.screen());
    let mut tail = fresh.app.feed.history_lines().to_vec();
    tail.drain(..tail.len() - reply.len());
    assert_eq!(tail, reply);
    for w in ["book", "reader", "okuyucu", "sample"] {
        assert!(!text.to_lowercase().contains(w), "{w}: {text}");
    }
    let reply_words: std::collections::HashSet<String> = text
        .split(|c: char| !c.is_alphabetic())
        .map(str::to_lowercase)
        .collect();
    let from_text: Vec<&String> = words
        .iter()
        .filter(|w| {
            reply_words.contains(*w) && !["retries", "upload", "client"].contains(&w.as_str())
        })
        .collect();
    assert!(from_text.is_empty(), "{from_text:?}");
    let last = reply.iter().rev().find(|l| !l.plain().trim().is_empty());
    assert!(last.unwrap().plain().starts_with("⏺ "), "{text}");
}

#[test]
fn the_same_request_gets_the_same_reply() {
    let mut h = sample_with(30, agent(), 80, 24);
    let first = whole_reply(&mut h, REQUEST);
    h.press(&["space"]); // ends it
    h.press(&["space"]); // and reading goes on
    let again = whole_reply(&mut h, REQUEST);
    assert_eq!(first, again);
    let mut other = sample_with(30, agent(), 80, 24);
    let different = whole_reply(&mut other, "rename the config loader and update callers");
    assert_ne!(first, different);
    // 2-5 tool calls (a sub-agent batch counts too), then one remark
    let calls = |r: &[Line]| r.iter().filter(|l| l.plain().starts_with("⏺ ")).count();
    assert!((3..=6).contains(&calls(&first)), "{}", calls(&first));
}

#[test]
fn a_reading_key_or_esc_ends_the_reply_where_the_text_was() {
    for k in ["space", "j", "escape", "q"] {
        let mut h = sample_with(30, agent(), 80, 24);
        h.press(&["space"]);
        let (pos, offset, before) = (h.session().pos(), h.session().line_offset, h.visible());
        ask(&mut h, REQUEST);
        h.advance(Duration::from_secs(2));
        h.press(&[k]);
        assert!(h.app.reply.is_none(), "{k}");
        assert!(!h.app.should_quit() && !h.app.panel, "{k} did only that");
        assert_eq!(
            (h.session().pos(), h.session().line_offset),
            (pos, offset),
            "{k}"
        );
        assert_eq!(h.visible(), before, "{k}");
        h.press(&["space"]);
        assert_ne!(h.session().pos(), pos, "{k}: reading goes on");
    }
}

#[test]
fn the_status_keeps_spinning_while_the_reply_plays() {
    let mut h = sample_with(30, agent(), 80, 24);
    ask(&mut h, REQUEST);
    assert!(h.app.animating());
    assert!(h.app.next_deadline() <= h.now + ANIMATION_FRAME);
    let a = h.status();
    h.advance(ANIMATION_FRAME);
    assert_ne!(h.status(), a);
}

#[test]
fn auto_scroll_waits_for_the_reply_and_goes_on_after() {
    let mut h = sample_with(
        30,
        Config {
            auto: true,
            ..agent()
        },
        80,
        24,
    );
    ask(&mut h, REQUEST);
    assert!(h.app.timer_due().is_none(), "auto waits");
    let pos = h.session().pos();
    play_out(&mut h);
    assert!(h.app.reply.is_none());
    assert_eq!(h.session().pos(), pos, "no text streamed meanwhile");
    assert!(h.app.auto && h.app.timer_due().is_some(), "and resumes");
    assert!(!h.screen().contains(REQUEST));
}

#[test]
fn hiding_during_a_reply_shows_only_the_hidden_stream() {
    let mut h = sample_with(10, agent(), 80, 24);
    let words = text_words(&mut h);
    ask(&mut h, "zebra quokka wombat");
    h.advance(Duration::from_secs(3));
    h.press(&["`"]);
    assert!(h.app.reply.is_none());
    for _ in 0..20 {
        h.advance(Duration::from_millis(250));
    }
    let screen = h.screen();
    assert!(
        !screen.contains("zebra") && !screen.contains('╭'),
        "{screen}"
    );
    assert!(!shown(&h).contains("zebra"));
    assert_eq!(leaked(&h, &words), Vec::<String>::new());
    h.press(&["`"]);
    assert!(!h.screen().contains("zebra"));
    assert!(
        h.screen().contains("Try"),
        "back to the text, the reply gone"
    );
}

#[test]
fn an_empty_request_just_closes_the_box() {
    let mut h = sample_with(10, agent(), 80, 24);
    ask(&mut h, "   ");
    assert!(h.app.ask.is_none() && h.app.reply.is_none());
    assert!(h.screen().contains("Try"));
}

#[test]
fn the_box_and_the_report_keep_out_of_each_other_s_way() {
    let mut h = sample_with(10, agent(), 80, 24);
    h.press(&["i"]);
    type_text(&mut h, "Sort the imports");
    assert!(h.app.report.is_none(), "S is typed into the box");
    assert!(h.screen().contains("Sort the imports"));
    h.press(&["enter"]);
    assert!(h.app.reply.is_some());
    h.press(&["S"]); // a key during the reply only ends it
    assert!(h.app.reply.is_none());
    assert!(h.app.report.is_none());
    h.press(&["S"]); // now it opens
    assert!(h.app.report.is_some());
}

#[test]
fn no_text_stays_on_screen_while_a_reply_plays() {
    let mut h = sample_with(10, agent(), 90, 30);
    let words = text_words(&mut h);
    assert!(!leaked(&h, &words).is_empty(), "the text is up before");
    ask(&mut h, "tidy up the retry loop");
    assert!(h.app.reply.is_some());
    while h.app.reply.is_some() {
        assert!(leaked(&h, &words).is_empty(), "{:?}", leaked(&h, &words));
        h.advance(Duration::from_millis(250));
    }
    play_out(&mut h);
    assert!(!leaked(&h, &words).is_empty(), "and back after");
}
