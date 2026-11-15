//! Sections, search and marks.

use super::*;
use crate::ui::search::highlight;

#[test]
fn brackets_jump_between_sections() {
    let mut h = sections_harness();
    h.press(&["]"]);
    assert_eq!(h.session().pos(), (1, 0));
    h.press(&["]", "]"]);
    assert_eq!(h.session().pos(), (2, 0)); // no section after the last
    h.press(&["["]);
    assert_eq!(h.session().pos(), (1, 0));
    h.press(&["j", "j", "j"]);
    assert_ne!((h.session().pos(), h.session().line_offset), ((1, 0), 0));
    h.press(&["["]);
    assert_eq!((h.session().pos(), h.session().line_offset), ((1, 0), 0));
    h.press(&["["]);
    assert_eq!(h.session().pos(), (0, 0));
}

#[test]
fn slash_searches_and_highlights() {
    let mut h = Harness::sample(80, 24);
    h.press(&["/"]);
    assert!(h.app.search.is_open());
    type_text(&mut h, "number 7");
    assert!(h.screen().lines().last().unwrap().starts_with("/number 7"));
    h.press(&["enter"]);
    assert!(!h.app.search.is_open());
    assert_eq!(h.session().pos(), (0, 7));
    let hits: Vec<String> = h
        .visible()
        .iter()
        .flat_map(|l| l.segments.clone())
        .filter(|(_, s)| s.reverse)
        .map(|(t, _)| t)
        .collect();
    assert!(hits.contains(&"number 7".to_string()), "{hits:?}");
}

#[test]
fn f_and_shift_f_walk_matches_and_wrap() {
    let mut h = Harness::sample(80, 24);
    h.press(&["/"]);
    type_text(&mut h, "LOREM");
    h.press(&["enter"]);
    assert_eq!(h.session().pos(), (0, 0));
    h.press(&["f"]);
    assert_eq!(h.session().pos(), (0, 1));
    h.press(&["F", "F"]);
    assert_eq!(h.session().pos(), (0, 9)); // wrapped to the end
    assert!(h.status().contains("continuing"));
}

#[test]
fn missing_pattern_reports_and_stays() {
    let mut h = Harness::sample(80, 24);
    h.press(&["/"]);
    type_text(&mut h, "zzz");
    h.press(&["enter"]);
    assert_eq!(h.session().pos(), (0, 0));
    assert!(h.status().contains("Pattern not found"));
}

#[test]
fn escape_cancels_search_and_backtick_hides() {
    let mut h = Harness::sample(80, 24);
    h.press(&["/", "x", "escape"]);
    assert!(!h.app.search.is_open());
    h.press(&["j"]);
    assert!(h.session().line_offset == 1 || h.session().pos() != (0, 0));
    h.press(&["/", "`"]);
    assert!(h.app.panic && !h.app.search.is_open());
}

#[test]
fn quote_keys_jump_between_marks() {
    let mut h = Harness::sample(80, 24);
    h.app.bookmarks = vec![[0, 6], [0, 2]];
    h.press(&["'"]);
    assert_eq!(h.session().pos(), (0, 2));
    h.press(&["'"]);
    assert_eq!(h.session().pos(), (0, 6));
    h.press(&["'"]);
    assert_eq!(h.session().pos(), (0, 2)); // wraps
    h.press(&["\""]);
    assert_eq!(h.session().pos(), (0, 6));
}

#[test]
fn quote_without_marks_says_so() {
    let mut h = Harness::sample(80, 24);
    h.press(&["'"]);
    assert_eq!(h.session().pos(), (0, 0));
    assert!(h.status().contains("cache"));
}

#[test]
fn bookmark_is_added_once_saved_and_flashed() {
    let mut h = Harness::sample(100, 24);
    h.press(&["space", "b", "b"]);
    let pos = h.session().pos();
    assert_eq!(h.app.bookmarks, vec![[pos.0, pos.1]]);
    assert_eq!(h.status(), "cache saved");
    let id = h.session().book.id.clone();
    assert_eq!(
        load_progress(&h.state_dir())[&id].bookmarks,
        vec![[pos.0, pos.1]]
    );
    h.advance(FLASH + Duration::from_millis(1));
    assert_ne!(h.status(), "cache saved");
}

#[test]
fn highlight_splits_only_text_segments() {
    let line = Line::book(
        vec![
            ("  12 ".into(), crate::masks::palette::GUTTER),
            ("Alfa beta ALFA".into(), crate::masks::palette::FG.book()),
        ],
        (0, 0),
    );
    let out = highlight(&line, "alfa");
    let texts: Vec<(&str, bool)> = out
        .segments
        .iter()
        .map(|(t, s)| (t.as_str(), s.reverse))
        .collect();
    assert_eq!(
        texts,
        vec![
            ("  12 ", false),
            ("Alfa", true),
            (" beta ", false),
            ("ALFA", true)
        ]
    );
}
