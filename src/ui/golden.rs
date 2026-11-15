//! Golden digests of what every mask draws in every theme at every colour depth.
//!
//! Each entry hashes the (symbol, fg, bg, modifiers) of every cell of a fixed
//! sample: header, paragraphs at two widths, a chapter transition, filler, an
//! error, search highlighting and a half-typed line, drawn on the theme's base.
//! A change in any colour, modifier or character shows up as a changed digest.
//!
//! Refresh with `FCC_BLESS=1 cargo test golden` after an intended change.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line as TLine, Span, Text};
use ratatui::widgets::{Block, Paragraph, Widget};
use sha2::{Digest, Sha256};

use super::feed::partial;
use super::search::highlight;
use super::theme::{Depth, THEMES};
use crate::book::Paragraph as Para;
use crate::errors::FccError;
use crate::masks::{LANGS, Line, Look, MASK_NAMES, get_mask, seeded};

const FIXTURE: &str = "src/ui/golden.txt";
const DEPTHS: &[(Depth, &str)] = &[
    (Depth::TrueColor, "truecolor"),
    (Depth::Ansi256, "ansi256"),
    (Depth::Ansi16, "ansi16"),
];

fn resolve(look: Look, theme: &str, depth: Depth) -> Style {
    super::style::style_in(look, theme, depth)
}

fn base(theme: &str, depth: Depth) -> Style {
    super::style::base_in(theme, depth)
}

/// The app's own chrome styles (input box, status line, side panel).
fn chrome_styles() -> Vec<(&'static str, Look)> {
    use super::app::{BOX_BORDER, FG, STATUS_FG};
    use super::panel::*;
    vec![
        ("box-border", BOX_BORDER),
        ("fg", FG),
        ("status", STATUS_FG),
        ("panel-border", PANEL_BORDER),
        ("panel", PANEL),
        ("panel-bold", PANEL_BOLD),
        ("panel-key", PANEL_KEY),
        ("panel-text", PANEL_TEXT),
        ("panel-dim", PANEL_DIM),
    ]
}

/// The fixed sample of lines a mask draws.
fn sample(name: &str, lang: &str) -> Vec<Line> {
    let mut m = get_mask(name, lang).unwrap();
    let mut rng = seeded(&format!("golden:{name}:{lang}"));
    let paras = [
        Para::new("Raskolnikov o sabah erkenden uyandı. Oda dar ve basıktı, ".repeat(3)),
        Para::new("漢字かな交じり文の例です。🙂 これは幅の広い文字を含みます。".repeat(2)),
        Para::new("kelime ".repeat(40)),
        Para::new("x".repeat(300)),
    ];
    let mut out = m.header(&mut rng, 80);
    let mut line_no = 1;
    for width in [80, 40] {
        for (i, p) in paras.iter().enumerate() {
            let lines = m.render(p, (0, i), width, &mut rng, line_no);
            line_no += lines.len();
            out.extend(lines);
        }
    }
    out.extend(m.transition(2, 80, line_no));
    out.extend((0..30).map(|_| m.noise(&mut rng, 80)));
    if let Some(block) = m.noise_block(12, &mut rng, 80) {
        out.extend(block);
    }
    out.extend(m.error(&FccError::NoTextLayer("Metin katmanı yok".into()), 80));
    let first = m.render(&paras[0], (0, 0), 80, &mut rng, 1);
    out.extend(first.iter().map(|l| highlight(l, "oda")));
    out.extend(first.iter().map(|l| partial(l, 7)));
    out
}

/// Every mask, every language of the code mask, plus the fake shell's listing.
fn samples() -> Vec<(String, Vec<Line>)> {
    let mut out = vec![];
    for name in MASK_NAMES {
        let langs: &[&str] = if *name == "code" { LANGS } else { &["python"] };
        for lang in langs {
            out.push((format!("{name}/{lang}"), sample(name, lang)));
        }
    }
    let mut m = get_mask("log", "python").unwrap();
    let mut rng = seeded("golden:shell");
    let mut shell = vec![];
    for cmd in ["ls", "git status"] {
        shell.extend(super::shell::shell_output(cmd, m.as_mut(), &mut rng, 80).unwrap());
    }
    out.push(("shell".into(), shell));
    out
}

fn hash_cells(buf: &Buffer) -> String {
    let mut h = Sha256::new();
    for cell in buf.content() {
        h.update(
            format!(
                "{}\u{0}{:?}\u{0}{:?}\u{0}{}\n",
                cell.symbol(),
                cell.fg,
                cell.bg,
                cell.modifier.bits()
            )
            .as_bytes(),
        );
    }
    short(h)
}

fn short(h: Sha256) -> String {
    h.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn draw(lines: &[Line], theme: &str, depth: Depth) -> Buffer {
    let area = Rect::new(0, 0, 80, lines.len() as u16);
    let mut buf = Buffer::empty(area);
    Block::default()
        .style(base(theme, depth))
        .render(area, &mut buf);
    let text = Text::from(
        lines
            .iter()
            .map(|l| {
                TLine::from(
                    l.segments
                        .iter()
                        .map(|(t, s)| Span::styled(t.clone(), resolve(*s, theme, depth)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
    );
    Paragraph::new(text).render(area, &mut buf);
    buf
}

fn digests() -> Vec<String> {
    let samples = samples();
    let mut out = vec![];
    for theme in THEMES {
        for (depth, depth_name) in DEPTHS {
            for (label, lines) in &samples {
                let d = hash_cells(&draw(lines, theme, *depth));
                out.push(format!("{label} {theme} {depth_name} {d}"));
            }
            let mut h = Sha256::new();
            for (name, s) in chrome_styles() {
                let st = resolve(s, theme, *depth);
                h.update(
                    format!(
                        "{name}\u{0}{:?}\u{0}{:?}\u{0}{}\u{0}{}\n",
                        st.fg,
                        st.bg,
                        st.add_modifier.bits(),
                        st.sub_modifier.bits()
                    )
                    .as_bytes(),
                );
            }
            out.push(format!("chrome {theme} {depth_name} {}", short(h)));
        }
    }
    out
}

#[test]
fn every_mask_draws_the_same_in_every_theme_and_depth() {
    let got = digests().join("\n") + "\n";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    if std::env::var_os("FCC_BLESS").is_some() {
        std::fs::write(&path, &got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap();
    let diff: Vec<String> = got
        .lines()
        .zip(want.lines())
        .filter(|(g, w)| g != w)
        .map(|(g, w)| format!("got {g}, want {w}"))
        .collect();
    assert!(diff.is_empty(), "{}", diff.join("\n"));
    assert_eq!(got.lines().count(), want.lines().count());
}

#[test]
fn the_sample_has_text_and_differs_between_themes() {
    let all = digests();
    assert_eq!(
        all.len(),
        THEMES.len() * DEPTHS.len() * (samples().len() + 1)
    );
    let dark = all
        .iter()
        .find(|l| l.starts_with("code/python dark truecolor"));
    let light = all
        .iter()
        .find(|l| l.starts_with("code/python light truecolor"));
    let tail = |l: Option<&String>| l.unwrap().rsplit(' ').next().unwrap().to_string();
    assert_ne!(tail(dark), tail(light));
}
