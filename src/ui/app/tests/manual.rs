//! The manual screen: paging, scrolling, saving, resizing and errors.

use super::*;

// -- test_app_manual.py --

#[test]
fn manual_space_advances_and_status_updates() {
    let mut h = Harness::sample(80, 24);
    assert_eq!(h.session().progress().0, 1);
    h.press(&["space"]);
    let done = h.session().progress().0;
    assert!(done > 1);
    assert!(
        h.status().contains(&format!(" {done}/10")),
        "{}",
        h.status()
    );
}

#[test]
fn space_starts_next_screen_at_first_unseen_line() {
    let mut h = Harness::sample(80, 24);
    let header = get_mask("code", "python")
        .unwrap()
        .header(&mut seeded("x"), 80);
    let stream = h.session().window(1000);
    let mut want = header.clone();
    want.extend(stream[..23 - header.len()].iter().cloned());
    assert_eq!(h.visible(), want);
    h.press(&["space"]);
    assert_eq!(
        h.visible(),
        stream[23 - header.len()..46 - header.len()].to_vec()
    );
    let s = h.session();
    let (pos, off) = (s.pos(), s.line_offset);
    assert_eq!(s.paragraph_lines(pos)[off], stream[23 - header.len()]);
    assert_ne!(pos, (0, 0));
}

#[test]
fn down_and_j_scroll_one_line_up_and_k_back() {
    let mut h = Harness::sample(80, 24);
    let stream = h.session().window(1000);
    h.press(&["down"]);
    assert_eq!(h.visible()[..23], stream[1..24]);
    h.press(&["j"]);
    assert_eq!(h.visible()[..23], stream[2..25]);
    h.press(&["down", "up"]);
    assert_eq!(h.visible(), stream[2..25].to_vec());
    h.press(&["k"]);
    assert_eq!(h.visible(), stream[1..24].to_vec());
}

#[test]
fn q_saves_progress_and_reopening_resumes() {
    let mut h = Harness::sample(80, 24);
    h.press(&["space", "space"]);
    let pos = h.session().pos();
    assert_ne!(pos, (0, 0));
    let id = h.session().book.id.clone();
    h.press(&["q"]);
    assert!(h.app.should_quit());
    let saved = load_progress(&h.state_dir());
    assert_eq!((saved[&id].chapter, saved[&id].paragraph), pos);
    let again = App::new(
        Some(h.dir.join("sample.txt")),
        cfg(),
        Theme::default(),
        h.state_dir(),
        80,
        24,
        h.now,
    );
    assert_eq!(again.session.as_ref().unwrap().pos(), pos);
}

#[test]
fn ctrl_q_also_saves() {
    let mut h = Harness::sample(80, 24);
    h.press(&["space", "space", "ctrl+q"]);
    assert!(h.app.should_quit());
    let pos = h.session().pos();
    let id = h.session().book.id.clone();
    let saved = load_progress(&h.state_dir());
    assert_eq!((saved[&id].chapter, saved[&id].paragraph), pos);
}

#[test]
fn resize_keeps_paragraph() {
    let mut h = Harness::sample(80, 24);
    h.press(&["down", "down"]);
    let pos = h.session().pos();
    h.app.resize(50, 24);
    assert_eq!(h.session().pos(), pos);
    assert_eq!(h.session().width(), 50);
    let lines = h.visible();
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|l| unicode_width::UnicodeWidthStr::width(l.plain().as_str()) <= 50)
    );
    assert!(lines.iter().any(|l| l.pos == Some(pos)));
}

#[test]
fn narrow_terminal_shows_one_warning() {
    let h = Harness::sample(38, 24);
    let lines = h.visible();
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.plain().contains("warning"))
            .count(),
        1
    );
    assert!(
        lines
            .iter()
            .all(|l| unicode_width::UnicodeWidthStr::width(l.plain().as_str()) <= 38)
    );
    assert!(
        !Harness::sample(80, 24)
            .visible()
            .iter()
            .any(|l| l.plain().contains("warning"))
    );
}

#[test]
fn empty_input_shows_error_not_crash() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("empty.txt");
    std::fs::write(&p, "").unwrap();
    let mut h = Harness::new(Some(p), cfg(), d, 80, 24);
    assert!(h.app.session.is_none());
    assert!(
        h.visible()
            .iter()
            .any(|l| l.plain().contains("empty input"))
    );
    h.press(&["space", "down", "up", "u", "q"]); // must not crash
}

#[test]
fn unknown_mask_falls_back_to_code_with_error() {
    let d = tempfile::tempdir().unwrap();
    let p = make_txt(d.path(), 10);
    let config = Config {
        mask: "nope".into(),
        lang: "rust".into(),
        noise: 0,
        ..Config::default()
    };
    let mut h = Harness::new(Some(p), config, d, 80, 24);
    assert_eq!(h.session().mask().name(), "code");
    assert!(
        h.app
            .feed
            .history_lines()
            .iter()
            .any(|l| l.plain().starts_with("error:"))
    );
    assert!(h.visible().iter().any(|l| l.is_book));
}

#[test]
fn every_cell_gets_the_themes_background() {
    use ratatui::style::Color;
    let h = Harness::sample(80, 24);
    for (name, bg) in [
        ("light", Color::Rgb(0xff, 0xff, 0xff)),
        ("dark", Color::Rgb(0x1e, 0x1e, 0x1e)),
        ("dark-ansi", Color::Reset),
        ("light-ansi", Color::Reset),
    ] {
        let buf = h.cells(name);
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                assert_eq!(buf[(x, y)].bg, bg, "{name} at {x},{y}");
            }
        }
    }
}

#[test]
fn screen_shows_no_marker_and_no_telling_words() {
    let h = Harness::sample(80, 24);
    let screen = h.screen().to_lowercase();
    assert!(screen.contains("lorem"));
    for w in ["book", "reader", "okuyucu"] {
        assert!(!screen.contains(w), "{w}");
    }
}

#[test]
fn page_back_returns_to_the_previous_screens() {
    let mut h = Harness::sample(80, 24);
    let first = h.visible();
    h.press(&["space"]);
    let second = h.visible();
    h.press(&["space", "pageup"]);
    assert_eq!(h.visible(), second);
    h.press(&["u"]);
    assert_eq!(h.visible(), first);
    h.press(&["u"]);
    assert_eq!(h.visible(), first);
    h.press(&["space"]);
    let after_space = h.visible();
    h.press(&["u", "pagedown"]);
    assert_eq!(h.visible(), after_space);
}

#[test]
fn autosave_writes_progress_every_ten_seconds() {
    let mut h = Harness::sample(80, 24);
    h.press(&["space"]);
    assert!(load_progress(&h.state_dir()).is_empty());
    h.advance(Duration::from_secs(11));
    assert_eq!(load_progress(&h.state_dir()).len(), 1);
}
