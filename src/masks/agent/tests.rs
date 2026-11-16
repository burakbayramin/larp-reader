use super::draw::plural;
use super::figure::{SPRITE_DROOP, SPRITE_MARCH, SPRITE_RAISED, SPRITE_SWUNG};
use super::reply::NO_CHANGE;
use super::welcome::WELCOME_MIN;
use super::*;
use rand::SeedableRng;
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

fn para_of_kind(kind: Kind, start: usize) -> Paragraph {
    (start..start + 500)
        .map(|i| format!("Sabah {i} erkenden uyandı, pencereyi açtı ve dışarı baktı. ").repeat(3))
        .find(|t| layout(t).kind == kind)
        .map(Paragraph::new)
        .expect("found")
}

fn rng(seed: u64) -> Rng {
    Rng::seed_from_u64(seed)
}

/// A numbered diff line: a line number, then `+ `, `- ` or two spaces, then code.
fn is_code_line(p: &str) -> bool {
    let c: Vec<char> = p.chars().collect();
    c.len() > 10
        && c[..7]
            .iter()
            .collect::<String>()
            .trim()
            .parse::<usize>()
            .is_ok()
        && c[7] == ' '
        && matches!((c[8], c[9]), ('+' | '-' | ' ', ' '))
}

fn has_bg(l: &Line, bg: Role) -> bool {
    l.segments
        .iter()
        .any(|(_, s)| s.bg == Some(crate::masks::Ink::Role(bg)))
}

// -- test_agent.py --

#[test]
fn some_paragraphs_come_after_a_batch_of_sub_agents() {
    let kinds: Vec<Kind> = (0..300)
        .map(|i| layout(&format!("paragraf {i}")).kind)
        .collect();
    let teams = kinds.iter().filter(|k| **k == Kind::Team).count();
    let says = kinds.iter().filter(|k| **k == Kind::Say).count();
    assert!((20..80).contains(&teams), "{teams}");
    assert!((20..80).contains(&says), "{says}");

    let mut m = AgentMask::default();
    for start in [0, 40, 90] {
        let p = para_of_kind(Kind::Team, start);
        for width in [40, 80, 120] {
            let lines = m.render(&p, (0, 0), width, &mut rng(1), 40);
            assert_eq!(lines.len(), m.line_count(&p, width), "{width}");
            let n = layout(&p.text).agents;
            assert!(lines[0].plain().contains(&format!("{n} agents finished")));
            assert!(lines[n].plain().trim_start().starts_with("└─"));
            assert!(lines[1..=n].iter().all(|l| !l.is_book));
            let shown: String = lines
                .iter()
                .flat_map(|l| l.segments.iter())
                .filter(|(_, look)| look.book)
                .map(|(t, _)| t.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let words = |t: &str| t.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
            assert_eq!(words(&shown), words(&p.text));
            for l in &lines {
                assert!(l.plain().width() <= width, "{width}: {}", l.plain());
            }
        }
    }
}

#[test]
fn the_session_now_and_then_runs_agents_and_writes_files() {
    let mut m = AgentMask::default();
    let mut r = rng(7);
    let stream: Vec<String> = (0..600).map(|_| m.noise(&mut r, 100).plain()).collect();
    assert!(stream.iter().any(|l| l.contains("agents…")), "agents");
    assert!(stream.iter().any(|l| l.contains("Write(")), "write");
    for l in &stream {
        for w in ["book", "reader", "okuyucu"] {
            assert!(!l.to_lowercase().contains(w), "{l}");
        }
    }
}

#[test]
fn layout_is_picked_from_the_text_and_mixes_kinds() {
    let edits = (0..200)
        .filter(|i| layout(&format!("paragraf {i}")).kind == Kind::Edit)
        .count();
    assert!((110..170).contains(&edits), "{edits}");
    assert_eq!(layout("aynı metin"), layout("aynı metin"));
}

#[test]
fn edit_paragraph_is_a_diff_with_the_text_as_added_lines() {
    let p = para_of_kind(Kind::Edit, 0);
    let lines = AgentMask::default().render(&p, (0, 0), 100, &mut rng(1), 40);
    assert!(lines[0].plain().starts_with(&format!("{BULLET} Update(")));
    assert!(lines[1].plain().contains("⎿  Updated ") && lines[1].plain().contains(" with "));
    let added: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
    assert!(!added.is_empty());
    assert!(added.iter().all(
        |l| l.plain().chars().skip(8).take(2).collect::<String>() == "+ " && has_bg(l, ADD_BG)
    ));
    assert_eq!(
        lines.iter().filter(|l| has_bg(l, DEL_BG)).count(),
        layout(&p.text).removed
    );
    assert_eq!(lines.last().unwrap().plain(), "");
    let summary = lines[1].plain();
    assert!(
        summary.contains(&format!("{} addition", added.len())),
        "{summary}"
    );
    assert!(
        summary.contains(&format!("{} removal", layout(&p.text).removed)),
        "{summary}"
    );
}

#[test]
fn say_paragraph_is_narration() {
    let lines =
        AgentMask::default().render(&para_of_kind(Kind::Say, 0), (0, 0), 100, &mut rng(1), 40);
    let text: Vec<&Line> = lines.iter().filter(|l| l.is_book).collect();
    assert!(text[0].plain().starts_with(&format!("{BULLET} ")));
    assert!(text[1..].iter().all(|l| l.plain().starts_with("  ")));
    assert_eq!(lines.last().unwrap().plain(), "");
}

#[test]
fn same_paragraph_draws_the_same_every_time() {
    let p = para_of_kind(Kind::Edit, 0);
    let a = AgentMask::default().render(&p, (0, 3), 90, &mut rng(5), 120);
    let b = AgentMask::default().render(&p, (0, 3), 90, &mut rng(5), 120);
    assert_eq!(a, b);
}

#[test]
fn diff_code_does_not_repeat_across_fifty_paragraphs() {
    let mut m = AgentMask::default();
    let mut seen: Vec<String> = vec![];
    let mut start = 1;
    for i in 0..50 {
        let lines = m.render(
            &para_of_kind(Kind::Edit, i * 7),
            (0, i),
            100,
            &mut rng(i as u64),
            start,
        );
        start += lines.len();
        for l in lines.iter().filter(|l| !l.is_book) {
            let p = l.plain();
            if is_code_line(&p) {
                seen.push(p.chars().skip(10).collect::<String>().trim().to_string());
            }
        }
    }
    let unique: std::collections::HashSet<&String> = seen.iter().collect();
    assert!(seen.len() > 100 && unique.len() == seen.len());
}

#[test]
fn hidden_stream_does_not_repeat_within_300_lines() {
    let mut m = AgentMask::default();
    let mut r = rng(9);
    let mut last: HashMap<String, usize> = HashMap::new();
    for i in 0..3000 {
        let p = m.noise(&mut r, 100).plain();
        let s = p.trim();
        let meaningful = (s.starts_with(BULLET) && s.contains('('))
            || is_code_line(&p)
            || s.contains('☐')
            || s.contains('☒');
        if meaningful && let Some(prev) = last.insert(s.to_string(), i) {
            assert!(i - prev > 300, "{s} after {}", i - prev);
        }
    }
}

#[test]
fn noise_block_has_exact_size_and_whole_units() {
    for count in 0..=10 {
        let mut m = AgentMask::default();
        m.render(&para_of_kind(Kind::Edit, 0), (0, 0), 100, &mut rng(1), 40);
        let block = m.noise_block(count, &mut rng(count as u64), 100).unwrap();
        assert_eq!(block.len(), count);
        assert!(!block.iter().any(|l| l.is_book));
        if let Some(first) = block.first() {
            assert!(!first.plain().trim_start().starts_with('⎿')); // a result always follows its call
        }
    }
}

#[test]
fn lines_fit_and_diff_background_spans_the_width() {
    let lines =
        AgentMask::default().render(&para_of_kind(Kind::Edit, 0), (0, 0), 80, &mut rng(1), 40);
    assert!(lines.iter().all(|l| l.plain().width() <= 80));
    assert!(
        lines
            .iter()
            .filter(|l| l.is_book)
            .all(|l| l.plain().width() == 80)
    );
}

#[test]
fn wide_text_diff_background_spans_the_width() {
    let text = |i: usize| {
        format!("{i} 漢字かな交じり文の例です。🙂 これは幅の広い文字を含みます。").repeat(4)
    };
    let para = (0..500)
        .map(text)
        .find(|t| layout(t).kind == Kind::Edit)
        .map(Paragraph::new)
        .expect("found");
    for width in [40, 60, 80] {
        let lines = AgentMask::default().render(&para, (0, 0), width, &mut rng(1), 40);
        assert!(lines.iter().all(|l| l.plain().width() <= width));
        assert!(
            lines
                .iter()
                .filter(|l| l.is_book)
                .all(|l| l.plain().width() == width),
            "{width}"
        );
    }
}

#[test]
fn the_figure_s_frames_line_up_and_move_on_time() {
    let cells = SPRITE[0].width();
    let all = SPRITE
        .iter()
        .chain(&SPRITE_RAISED)
        .chain(&SPRITE_SWUNG)
        .chain(&[SPRITE_DROOP])
        .chain(&SPRITE_MARCH);
    for f in all {
        assert_eq!(f.width(), cells, "{f:?}");
    }
    for (i, a) in SPRITE.iter().enumerate() {
        assert!(SPRITE.iter().skip(i + 1).all(|b| a != b), "rows differ");
    }
    let shown = |ms: u128, active: bool| -> Vec<&str> {
        sprite_frame(Duration::from_millis(ms as u64), active)
            .into_iter()
            .map(|(_, now)| now)
            .collect()
    };
    assert_eq!(shown(0, false), SPRITE.to_vec(), "proud pose");
    let every = SHOW_EVERY.as_millis();
    let from = (SHOW_EVERY - SHOW_FOR).as_millis();
    let half = SHOW_FOR.as_millis() / 2;
    assert_eq!(shown(from + 10, false)[..2], SPRITE_RAISED);
    assert_eq!(shown(from + half + 10, false)[..2], SPRITE_SWUNG);
    assert_eq!(
        shown(every + 10, false),
        SPRITE.to_vec(),
        "back to the pose"
    );
    let droop = 2 * every + from + 10;
    assert_eq!(shown(droop, false)[0], SPRITE_DROOP);
    assert_eq!(shown(droop, false)[1..], SPRITE[1..]);
    assert_eq!(
        shown(3 * every + 10, false),
        SPRITE.to_vec(),
        "straightened"
    );
    let step = STEP_FOR.as_millis();
    assert_eq!(shown(step + 10, false)[3], SPRITE[3], "still while idle");
    assert_eq!(shown(10, true)[3], SPRITE_MARCH[0]);
    assert_eq!(shown(step + 10, true)[3], SPRITE_MARCH[1]);
    assert_eq!(shown(from + 10, true)[..3], SPRITE[..3], "sword stays up");
}

#[test]
fn the_welcome_box_fits_or_stays_out() {
    for width in [WELCOME_MIN, 40, 80, 200] {
        let lines = welcome(width);
        assert_eq!(lines.len(), SPRITE.len() + 3);
        for l in &lines {
            assert!(l.plain().width() <= width, "{width}: {}", l.plain());
        }
        let box_w = lines[0].plain().width();
        let boxed = &lines[..SPRITE.len() + 2];
        assert!(boxed.iter().all(|l| l.plain().width() == box_w));
        let all: String = lines.iter().map(|l| l.plain()).collect();
        for w in ["book", "reader", "okuyucu"] {
            assert!(!all.to_lowercase().contains(w));
        }
    }
    assert!(welcome(WELCOME_MIN - 1).is_empty());
}

#[test]
fn header_and_status() {
    let mut m = AgentMask::default();
    let header = m.header(&mut rng(1), 80);
    assert!(header[0].plain().starts_with('╭'));
    assert!(header.iter().any(|l| l.plain().starts_with('>')));
    let narrow = m.header(&mut rng(1), WELCOME_MIN - 1);
    assert!(narrow[0].plain().starts_with('>'));
    assert!(m.progress_label(142, 380, 80).contains("142/380"));
}

#[test]
fn spinner_frames_change_with_time() {
    let at = |ms| spinner_frame(Duration::from_millis(ms));
    assert_eq!(at(0), "·");
    assert_eq!(at(119), "·");
    assert_eq!(at(120), "✢");
    let cycle: Vec<&str> = (0..SPINNER.len() as u64).map(|i| at(i * 120)).collect();
    assert_eq!(cycle, SPINNER);
    assert_eq!(at(SPINNER.len() as u64 * 120), "·"); // and round again
    let m = AgentMask::default();
    let label = |ms| {
        m.animated_label(3, 10, 80, Duration::from_millis(ms))
            .unwrap()
    };
    assert_ne!(label(0), label(120));
    assert!(label(0).starts_with("· "));
}

#[test]
fn verbs_are_deterministic_and_change_every_few_seconds() {
    let at = |s| verb(Duration::from_secs(s));
    assert_eq!(at(0), at(3));
    assert_eq!(at(0), verb(Duration::from_millis(3999)));
    let mut seen = std::collections::HashSet::new();
    for slot in 0..VERBS.len() as u64 {
        let (here, next) = (at(slot * 4), at(slot * 4 + 4));
        assert_ne!(here, next);
        seen.insert(here);
    }
    assert_eq!(seen.len(), VERBS.len()); // every verb comes up
    let again: Vec<&str> = (0..50).map(at).collect();
    assert_eq!(again, (0..50).map(at).collect::<Vec<_>>());
}

#[test]
fn animated_label_counts_seconds_and_tokens_and_fits() {
    let m = AgentMask::default();
    let l = m
        .animated_label(5, 380, 80, Duration::from_secs(12))
        .unwrap();
    assert!(l.contains("(12s · 5/380 · ↓ "), "{l}");
    assert!(l.ends_with(" tokens)"), "{l}");
    let tokens = |secs| {
        let l = m
            .animated_label(5, 380, 200, Duration::from_secs(secs))
            .unwrap();
        let t = l.split("↓ ").nth(1).unwrap().split(' ').next().unwrap();
        let k = t
            .strip_suffix('k')
            .map(|n| n.parse::<f64>().unwrap() * 1000.0);
        k.unwrap_or_else(|| t.parse().unwrap())
    };
    assert!(tokens(60) > tokens(1));
    assert!(
        m.animated_label(5, 380, 80, Duration::from_secs(185))
            .unwrap()
            .contains("(3m 5s ·")
    );
    let narrow = m
        .animated_label(5, 380, 30, Duration::from_secs(1))
        .unwrap();
    assert!(narrow.chars().count() <= 30, "{narrow}");
    assert!(narrow.contains("5/380"));
    assert!(!l.contains("esc"));
}

#[test]
fn the_reply_s_summary_counts_the_files_it_showed() {
    let m = AgentMask::default();
    let (mut none, mut some) = (0, 0);
    for i in 0..80 {
        let request = format!("look at case {i}");
        let lines: Vec<String> = m
            .reply(&request, 100)
            .into_iter()
            .flatten()
            .map(|l| l.plain())
            .collect();
        let files = lines
            .iter()
            .filter(|l| l.contains("Update(") || l.contains("Write("))
            .count();
        let last = lines.iter().rev().find(|l| l.starts_with(BULLET)).unwrap();
        if files == 0 {
            none += 1;
            assert!(NO_CHANGE.iter().any(|t| last.ends_with(t)), "{last}");
        } else {
            some += 1;
            assert!(last.contains(&plural(files, "file")), "{files}: {last}");
        }
    }
    assert!(none > 0 && some > 0, "{none} {some}");
}

#[test]
fn replies_are_a_few_tool_calls_that_fit_and_repeat() {
    let m = AgentMask::default();
    let mut sizes = std::collections::HashSet::new();
    for i in 0..60 {
        let request = format!("fix issue {i} in the parser{}", " please".repeat(i % 12));
        for width in [40, 80, 160] {
            let groups = m.reply(&request, width);
            assert_eq!(groups, m.reply(&request, width), "same request, same reply");
            // echo, thinking, 2-5 calls (a sub-agent batch among them), a remark
            let calls = groups.len() - 3;
            assert!((2..=5).contains(&calls), "{calls}");
            sizes.insert(calls);
            assert!(groups[0][0].plain().starts_with("> fix issue"));
            assert_eq!(groups[1][0].plain(), THINKING);
            let end = groups.last().unwrap()[0].plain();
            assert!(end.starts_with(&format!("{BULLET} ")), "{end}");
            for l in groups.iter().flatten() {
                assert!(!l.is_book && l.pos.is_none());
                assert!(l.plain().width() <= width, "{}", l.plain());
                let low = l.plain().to_lowercase();
                assert!(
                    !["book", "reader", "okuyucu"]
                        .iter()
                        .any(|w| low.contains(w))
                );
            }
        }
    }
    assert_eq!(sizes.len(), 4, "{sizes:?}");
    let long = m.reply(
        "please refactor the session store so it can be shared by workers",
        80,
    );
    assert!(
        long.iter()
            .flatten()
            .any(|l| l.plain().contains("agents finished"))
    );
}
