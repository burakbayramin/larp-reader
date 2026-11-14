//! The side panel and the folder's files.

use super::*;

fn panel(h: &Harness) -> String {
    h.app
        .panel_text()
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn escape_toggles_a_side_panel_with_keys_and_files() {
    let mut h = shelf();
    h.press(&["escape"]);
    assert!(h.app.panel);
    let text = panel(&h);
    assert!(text.contains("page down") && text.contains("hide / unhide"));
    assert!(
        text.contains("alpha.txt") && text.contains("> beta.txt") && text.contains("gamma.txt")
    );
    assert!(!text.contains("cover.jpg") && !text.contains("notes"));
    assert!(h.screen().contains("page down")); // drawn over the text
    h.press(&["escape"]);
    assert!(!h.app.panel);
}

#[test]
fn backtick_hides_the_panel_too_and_escape_does_not_undo_it() {
    let mut h = shelf();
    h.press(&["escape", "`"]);
    assert!(h.app.panic && !h.app.panel);
    h.press(&["escape"]);
    assert!(h.app.panic && !h.app.panel);
}

#[test]
fn o_and_shift_o_walk_the_folder_and_keep_each_place() {
    let mut h = shelf();
    h.press(&["space"]);
    let beta_pos = h.session().pos();
    h.press(&["o"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("gamma")
    );
    h.press(&["o"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("alpha")
    );
    h.press(&["O", "O"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("beta")
    );
    assert_eq!(h.session().pos(), beta_pos);
    assert_eq!(load_progress(&h.state_dir()).len(), 3);
}

#[test]
fn digits_open_a_listed_file_only_while_the_panel_is_open() {
    let mut h = shelf();
    h.press(&["1"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("beta")
    );
    h.press(&["escape", "1"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("alpha")
    );
    assert!(panel(&h).contains("> alpha.txt"));
}

#[test]
fn switching_files_is_ignored_while_hidden() {
    let mut h = shelf();
    h.press(&["`", "o"]);
    h.press(&["`"]);
    assert!(
        h.session()
            .book
            .paragraph((0, 0))
            .unwrap()
            .text
            .starts_with("beta")
    );
}
