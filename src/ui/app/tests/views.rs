//! Views, languages, filler, themes, and file names kept off screen.

use super::*;
use crate::masks::MASK_NAMES;

// -- test_app_switch.py, test_help_and_inputs.py, test_menu_bookmarks.py --

#[test]
fn m_and_shift_m_cycle_views_keeping_the_place() {
    let mut h = Harness::sample(100, 24);
    h.press(&["space"]);
    let pos = h.session().pos();
    let mut seen = vec![h.session().mask().name().to_string()];
    for _ in 0..MASK_NAMES.len() {
        h.press(&["m"]);
        seen.push(h.session().mask().name().to_string());
        assert_eq!(h.session().pos(), pos);
        assert!(h.visible().iter().any(|l| l.pos == Some(pos)));
        assert_eq!(
            crate::state::load_config(&h.state_dir()).mask,
            *seen.last().unwrap()
        );
    }
    assert_eq!(
        seen,
        [
            "code",
            "log",
            "diff",
            "test",
            "build",
            "git",
            "docker",
            "kubectl",
            "terraform",
            "sql",
            "jupyter",
            "agent",
            "code"
        ]
    );
    h.press(&["M"]);
    assert_eq!(h.session().mask().name(), "agent");
}

#[test]
fn l_cycles_languages_and_returns_to_code() {
    let mut h = Harness::sample(100, 24);
    h.press(&["m", "l"]);
    assert_eq!(
        (h.session().mask().name(), h.app.config.lang.as_str()),
        ("code", "typescript")
    );
    h.press(&["L", "L"]);
    assert_eq!(h.app.config.lang, "csharp");
    assert_eq!(crate::state::load_config(&h.state_dir()).lang, "csharp");
}

#[test]
fn n_and_shift_n_change_filler_within_bounds() {
    let mut h = Harness::sample(100, 24);
    h.press(&["n", "n"]);
    assert_eq!((h.app.config.noise, h.session().noise()), (2, 2));
    h.press(&["n"; 9]);
    assert_eq!(h.app.config.noise, 5);
    h.press(&["N"; 9]);
    assert_eq!(h.app.config.noise, 0);
    assert_eq!(crate::state::load_config(&h.state_dir()).noise, 0);
}

// -- themes --

#[test]
fn t_cycles_themes_and_saves_the_choice() {
    let mut h = Harness::sample(80, 24);
    assert_eq!(h.app.config.theme, "monokai");
    h.press(&["t"]);
    assert_eq!(h.app.config.theme, "dark");
    assert_eq!(crate::state::load_config(&h.state_dir()).theme, "dark");
    h.press(&["t"; 6]);
    assert_eq!(h.app.config.theme, "monokai"); // round the seven themes
    h.press(&["`", "t"]);
    assert_eq!(h.app.config.theme, "monokai"); // not while hidden
}

#[test]
fn the_app_draws_in_its_own_theme_and_t_keeps_the_depth() {
    use crate::ui::theme::Depth;
    use ratatui::style::Color;
    let mut h = Harness::sample(80, 24);
    h.app.theme = Theme::new("monokai", Depth::Ansi256);
    h.press(&["t", "t"]); // monokai, dark, light
    assert_eq!(h.app.theme, Theme::new("light", Depth::Ansi256));
    let mut t = Terminal::new(TestBackend::new(80, 24)).unwrap();
    t.draw(|f| h.app.draw(f)).unwrap();
    let light_bg = Theme::new("light", Depth::Ansi256).base().bg;
    assert!(matches!(light_bg, Some(Color::Indexed(_))));
    assert_eq!(Some(t.backend().buffer()[(0, 0)].bg), light_bg);
}

// -- test_no_filename_on_screen.py --

#[test]
fn bad_input_at_startup_hides_the_file_name() {
    let d = tempfile::tempdir().unwrap();
    let odd = d.path().join("SucVeCeza");
    std::fs::write(&odd, b"\x00\x01 just some opaque bytes \x02").unwrap();
    for path in [d.path().join("Suç ve Ceza.epub"), odd] {
        let app = App::new(
            Some(path),
            cfg(),
            Theme::default(),
            d.path().join("st"),
            80,
            24,
            Instant::now(),
        );
        let text: String = app
            .feed
            .history_lines()
            .iter()
            .map(Line::plain)
            .chain([app.status_text()])
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();
        assert!(text.contains("error:"), "{text}");
        for part in ["suç", "suc", "ceza", "epub"] {
            assert!(!text.contains(part), "{part} in {text}");
        }
    }
}

#[test]
fn the_status_line_is_coloured_and_has_the_view_s_extras_when_there_is_room() {
    use unicode_width::UnicodeWidthStr;
    for name in MASK_NAMES {
        let config = Config {
            mask: (*name).into(),
            ..cfg()
        };
        let h = sample_with(10, config.clone(), 120, 24);
        let segs = h.app.status_segments();
        let text: String = segs.iter().map(|(t, _)| t.as_str()).collect();
        assert!(text.starts_with(&h.app.status_text()), "{name}: {text}");
        assert!(text.width() < 120, "{name}: fits: {text}");
        assert!(
            text.width() > h.app.status_text().width(),
            "{name}: extras: {text}"
        );
        let looks: std::collections::HashSet<_> = segs.iter().map(|(_, l)| *l).collect();
        assert!(looks.len() >= 3, "{name}: colours: {segs:?}");
        for w in ["book", "reader", "okuyucu", "sample"] {
            assert!(!text.to_lowercase().contains(w), "{name}: {w}");
        }
        assert!(h.status().starts_with(&h.app.status_text()));

        // a narrow window keeps only the label
        let narrow = sample_with(10, config, 40, 24);
        let text: String = narrow
            .app
            .status_segments()
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        assert_eq!(text, narrow.app.status_text(), "{name}");
    }
    // a flash message stays plain
    let mut h = sample_with(
        10,
        Config {
            mask: "agent".into(),
            ..cfg()
        },
        120,
        24,
    );
    h.press(&["+"]);
    assert_eq!(h.app.status_segments().len(), 1);
}
