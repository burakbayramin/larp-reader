//! The settings screen.

use super::*;

#[test]
fn menu_changes_settings_and_saves_them() {
    let mut h = Harness::sample(100, 30);
    h.press(&[":"]);
    assert!(h.app.menu.is_some());
    let screen = h.screen().to_lowercase();
    assert!(screen.contains("fcc config") && screen.contains("target: < code >"));
    for w in ["book", "reader", "okuyucu"] {
        assert!(!screen.contains(w));
    }
    for name in ["target", "lang", "rate", "verbosity", "on hide"] {
        h.app.menu.as_mut().unwrap().focus(name);
        h.press(&["right"]);
    }
    h.press(&["enter"]);
    assert!(h.app.menu.is_none());
    assert_eq!(h.session().mask().name(), "log");
    assert_eq!(h.session().noise(), 1);
    let saved = crate::state::load_config(&h.state_dir());
    assert_eq!(
        (
            saved.mask.as_str(),
            saved.lang.as_str(),
            saved.noise,
            saved.wpm,
            saved.hide.as_str()
        ),
        ("log", "typescript", 1, 240, "shell")
    );
}

#[test]
fn menu_lists_sections_and_jumps_to_one() {
    let mut h = sections_harness();
    h.press(&[":"]);
    let menu = h.app.menu.as_mut().unwrap();
    assert_eq!(menu.sections().len(), 3);
    assert!(menu.sections()[2].0.contains("Kısım 2"));
    menu.focus("sections");
    h.press(&["down", "down", "enter"]);
    assert!(h.app.menu.is_none());
    assert_eq!(h.session().pos(), (2, 0));
}

#[test]
fn menu_escape_closes_backtick_hides_and_marks_jump() {
    let mut h = Harness::sample(100, 30);
    h.press(&["space", "space", "b", "u", "u"]);
    let mark = h.app.bookmarks[0];
    h.press(&[":"]);
    h.app.menu.as_mut().unwrap().focus("marks");
    h.press(&["enter"]);
    assert_eq!(h.session().pos(), (mark[0], mark[1]));
    h.press(&[":", "escape"]);
    assert!(h.app.menu.is_none() && !h.app.panic);
    h.press(&[":", "`"]);
    assert!(h.app.menu.is_none() && h.app.panic);
}

#[test]
fn menu_path_opens_a_file_and_a_bad_one_hides_its_name() {
    let mut h = shelf();
    let alpha = h.dir.join("shelf").join("alpha.txt");
    h.press(&[":"]);
    type_text(&mut h, alpha.to_str().unwrap());
    h.press(&["enter"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("alpha")
    );
    let missing = h.dir.join("Suç ve Ceza.epub");
    h.press(&[":"]);
    type_text(&mut h, missing.to_str().unwrap());
    h.press(&["enter"]);
    let text = h
        .visible()
        .iter()
        .map(Line::plain)
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
        + &h.status();
    assert!(text.contains("error:"));
    for part in ["suç", "suc", "ceza", "epub"] {
        assert!(!text.contains(part), "{part}");
    }
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("alpha")
    ); // still open
}

#[test]
fn menu_sets_the_theme() {
    let mut h = Harness::sample(100, 30);
    h.press(&[":"]);
    assert!(h.screen().contains("theme: < dark >"));
    h.app.menu.as_mut().unwrap().focus("theme");
    h.press(&["right", "right", "enter"]);
    assert_eq!(h.app.config.theme, "dark-daltonized");
    assert_eq!(
        crate::state::load_config(&h.state_dir()).theme,
        "dark-daltonized"
    );
}
