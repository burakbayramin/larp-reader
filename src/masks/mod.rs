//! The disguises text is shown in (`masks/__init__.py`, `masks/base.py`).

use rand::SeedableRng;

use crate::book::{Paragraph, Pos};
use crate::errors::FccError;

pub mod agent;
pub mod base;
pub mod build;
pub mod code;
pub mod codegen;
pub mod diff;
pub mod docker;
pub mod filler;
pub mod git;
pub mod log;
pub mod look;
pub mod palette;
pub mod section;
pub mod snippets;
pub mod test;
pub mod time;

pub use base::{Line, Segment};
pub use look::{Ink, Look, Role};

/// The random source masks draw filler from: seeded, so a paragraph draws the same every time.
pub type Rng = rand_pcg::Pcg32;

/// A generator seeded from text (FNV-1a), like Python's `Random("book:pos:mask:noise")`.
pub fn seeded(seed: &str) -> Rng {
    seeded_fmt(format_args!("{seed}"))
}

/// FNV-1a over everything written, so a seed can be hashed without being built.
struct Fnv(u64);

impl std::fmt::Write for Fnv {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        for b in s.bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(())
    }
}

/// `seeded(&format!(..))` without the string: the same seed, no allocation.
pub fn seeded_fmt(seed: std::fmt::Arguments) -> Rng {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    std::fmt::Write::write_fmt(&mut h, seed).expect("hashing cannot fail");
    Rng::seed_from_u64(h.0)
}

/// `random.choice`.
pub fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> &'a T {
    &items[rand::Rng::gen_range(rng, 0..items.len())]
}

/// `random.randint` (both ends included).
pub fn randint(rng: &mut Rng, a: usize, b: usize) -> usize {
    rand::Rng::gen_range(rng, a..=b)
}

/// `random.random`: in [0, 1).
pub fn random(rng: &mut Rng) -> f64 {
    rand::Rng::r#gen(rng)
}

/// `random.choices(items, weights)[0]`.
pub fn weighted<'a, T>(rng: &mut Rng, items: &'a [T], weights: &[u32]) -> &'a T {
    let total: u32 = weights.iter().sum();
    let mut x = rand::Rng::gen_range(rng, 0..total);
    for (item, w) in items.iter().zip(weights) {
        if x < *w {
            return item;
        }
        x -= w;
    }
    items.last().expect("non-empty")
}

/// In the order `m` cycles through them.
pub const MASK_NAMES: &[&str] = &[
    "code", "log", "diff", "test", "build", "git", "docker", "agent",
];

/// Source languages of the code mask.
pub const LANGS: &[&str] = &["python", "typescript", "go", "rust", "java", "csharp"];

pub trait Mask {
    fn name(&self) -> &'static str;

    /// Lines that open the output, shown above the first paragraph.
    fn header(&mut self, rng: &mut Rng, width: usize) -> Vec<Line>;

    /// A paragraph in disguise. `line_no_start` is the number of its first line,
    /// counted from the top of the text, for masks that number or time lines.
    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line>;

    /// How many lines `render` gives for this paragraph, without building them.
    fn line_count(&self, para: &Paragraph, width: usize) -> usize;

    /// Lines marking the start of chapter `chapter`, in the mask's own idiom (a new
    /// file, a merge commit, a container restart...). The session puts them above the
    /// first paragraph of every chapter but the first, as part of that paragraph.
    ///
    /// Names come from `section`, chosen by `chapter`: never the chapter's title.
    /// `line_no_start` is the number of the first of these lines, as in `render`;
    /// how many lines there are must not depend on it.
    fn transition(&self, chapter: usize, width: usize, line_no_start: usize) -> Vec<Line>;

    /// Where the paragraph about to be rendered sits in the text: `done`
    /// paragraphs come before it, out of `total`. The session says so before
    /// asking for its `transition`, for masks that show real progress there.
    fn set_progress(&mut self, _done: usize, _total: usize) {}

    /// One line of filler.
    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line;

    /// The next `noise` lines are the hidden screen's stream, not filler of
    /// the text: they must not repeat what was just around it. Called before
    /// each stream line; a render in between goes back to the text's filler.
    fn stream(&mut self) {}

    /// Exactly `count` lines of filler in whole units, for masks whose filler
    /// only makes sense that way; None means one `noise` line at a time.
    fn noise_block(&mut self, _count: usize, _rng: &mut Rng, _width: usize) -> Option<Vec<Line>> {
        None
    }

    fn error(&self, err: &FccError, width: usize) -> Vec<Line> {
        base::error_lines(err, width)
    }

    fn progress_label(&self, done: usize, total: usize, width: usize) -> String;

    /// A status label that moves with time, for masks that look like a live
    /// session; `elapsed` is how long the session has been up. None means the
    /// still `progress_label`. The app redraws often while this is Some.
    fn animated_label(
        &self,
        _done: usize,
        _total: usize,
        _width: usize,
        _elapsed: std::time::Duration,
    ) -> Option<String> {
        None
    }

    /// Whether streamed text lines type out a few characters at a time.
    fn typewriter(&self) -> bool {
        false
    }

    /// A faint placeholder for an (inert) input box drawn above the status
    /// line, for masks that look like an interactive session; None: no box.
    fn prompt_box(&self) -> Option<&'static str> {
        None
    }

    /// What the session "does" for a request typed into the input box:
    /// groups of lines played one after another (the request echoed, a
    /// thinking moment, the work, a closing remark). Chosen from the request
    /// alone, so the same request gets the same reply; never any text of the
    /// open file, and none of it is a text line. Empty: no reply.
    fn reply(&self, _request: &str, _width: usize) -> Vec<Vec<Line>> {
        vec![]
    }

    /// A little figure in the header that moves: pairs of (row as the header
    /// draws it, row to show now). The app swaps them in at draw time, so the
    /// header itself never changes. `active`: text is streaming. None: no figure.
    /// Extra status pieces drawn at the right end of the status line, like a
    /// tool's own status bar (branch, counters, mode). Dropped when the
    /// window is too narrow. Never carries text of the book.
    fn status_right(
        &self,
        _done: usize,
        _total: usize,
        _elapsed: std::time::Duration,
    ) -> Vec<base::Segment> {
        vec![]
    }

    fn sprite(
        &self,
        _elapsed: std::time::Duration,
        _active: bool,
    ) -> Option<Vec<(&'static str, &'static str)>> {
        None
    }
}

/// A fresh mask by name; `lang` matters only to the code mask.
pub fn get_mask(name: &str, lang: &str) -> Result<Box<dyn Mask>, String> {
    Ok(match name {
        "code" => Box::new(code::CodeMask::new(lang)?),
        "log" => Box::new(log::LogMask::default()),
        "diff" => Box::new(diff::DiffMask::default()),
        "test" => Box::new(test::TestMask::default()),
        "build" => Box::new(build::BuildMask::default()),
        "git" => Box::new(git::GitMask::default()),
        "docker" => Box::new(docker::DockerMask::default()),
        "agent" => Box::new(agent::AgentMask::default()),
        _ => return Err(format!("unknown or unregistered mask: {name:?}")),
    })
}

/// The rules every mask keeps (`tests/masks/test_contract.py`).
#[cfg(test)]
mod contract {
    use super::*;
    use crate::errors::FccError;
    use rand::SeedableRng;
    use unicode_width::UnicodeWidthStr;

    const FORBIDDEN: &[&str] = &["book", "reader", "okuyucu"];

    fn para() -> Paragraph {
        Paragraph::new("Raskolnikov o sabah erkenden uyandı. Oda dar ve basıktı, ".repeat(3))
    }

    fn wide_para() -> Paragraph {
        Paragraph::new("漢字かな交じり文の例です。🙂 これは幅の広い文字を含みます。".repeat(4))
    }

    fn rng(seed: u64) -> Rng {
        Rng::seed_from_u64(seed)
    }

    fn masks() -> Vec<Box<dyn Mask>> {
        MASK_NAMES
            .iter()
            .map(|n| get_mask(n, "python").unwrap())
            .collect()
    }

    /// Words of the text segments, in order.
    fn text_words(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .filter(|l| l.is_book)
            .flat_map(|l| &l.segments)
            .filter(|(_, s)| s.book)
            .flat_map(|(t, _)| t.split_whitespace().map(String::from))
            .collect()
    }

    #[test]
    fn render_keeps_every_word_in_order() {
        for mut m in masks() {
            let lines = m.render(&para(), (0, 0), 60, &mut rng(1), 1);
            let want: Vec<String> = para().text.split_whitespace().map(String::from).collect();
            assert_eq!(text_words(&lines), want, "{}", m.name());
        }
    }

    #[test]
    fn lines_fit_width() {
        for mut m in masks() {
            let lines = m.render(&para(), (0, 0), 60, &mut rng(1), 1);
            assert!(!lines.is_empty());
            for l in &lines {
                assert!(l.plain().width() <= 60, "{}: {}", m.name(), l.plain());
            }
        }
    }

    fn squeezed(lines: &[Line]) -> String {
        text_words(lines).concat()
    }

    #[test]
    fn wide_text_fits_width_and_keeps_every_word_in_order() {
        let want: String = wide_para().text.split_whitespace().collect();
        for width in [40, 60] {
            for name in MASK_NAMES {
                let mut m = get_mask(name, "python").unwrap();
                let lines = m.render(&wide_para(), (0, 0), width, &mut rng(1), 1);
                assert!(!lines.is_empty());
                for l in &lines {
                    assert!(
                        l.plain().width() <= width,
                        "{name} at {width}: {}",
                        l.plain()
                    );
                }
                // wide runs have no spaces to break at, so they are cut mid-word
                assert_eq!(squeezed(&lines), want, "{name} at {width}");
                assert_eq!(m.line_count(&wide_para(), width), lines.len(), "{name}");
                let again = get_mask(name, "python").unwrap().render(
                    &wide_para(),
                    (0, 0),
                    width,
                    &mut rng(1),
                    1,
                );
                assert_eq!(lines, again, "{name} is deterministic");
            }
        }
    }

    #[test]
    fn book_lines_carry_pos() {
        for mut m in masks() {
            let lines = m.render(&para(), (0, 0), 60, &mut rng(1), 1);
            assert!(lines.iter().any(|l| l.is_book));
            assert!(
                lines.iter().all(|l| (l.pos == Some((0, 0))) == l.is_book),
                "{}",
                m.name()
            );
        }
    }

    #[test]
    fn error_shows_real_reason() {
        let reason = "Metin katmanı yok, OCR desteklenmiyor";
        for m in masks() {
            let lines = m.error(&FccError::NoTextLayer(reason.into()), 60);
            assert!(
                lines
                    .iter()
                    .map(Line::plain)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .contains(reason)
            );
        }
    }

    #[test]
    fn no_forbidden_words_in_chrome() {
        for mut m in masks() {
            let mut r = rng(3);
            let mut chrome: Vec<String> = m.header(&mut r, 80).iter().map(Line::plain).collect();
            chrome.extend((0..50).map(|_| m.noise(&mut r, 50).plain()));
            chrome.push(m.progress_label(142, 380, 50));
            let text = chrome.join("\n").to_lowercase();
            assert!(!FORBIDDEN.iter().any(|w| text.contains(w)), "{}", m.name());
        }
    }

    #[test]
    fn render_tiny_width_does_not_hang() {
        for mut m in masks() {
            m.render(&para(), (0, 0), 3, &mut rng(1), 1);
        }
    }

    #[test]
    fn unknown_names_and_languages_are_errors() {
        assert!(get_mask("nope", "python").is_err());
        assert!(get_mask("code", "cobol").is_err());
    }

    #[test]
    fn header_fits_width() {
        for width in [40, 60, 120] {
            for mut m in masks() {
                let lines = m.header(&mut rng(1), width);
                assert!(!lines.is_empty());
                assert!(
                    lines
                        .iter()
                        .all(|l| l.plain().width() <= width && !l.is_book),
                    "{}",
                    m.name()
                );
            }
        }
    }

    /// Every mask, every language of the code mask.
    fn all_masks() -> Vec<(String, Box<dyn Mask>)> {
        MASK_NAMES
            .iter()
            .flat_map(|name| {
                let langs: &[&str] = if *name == "code" { LANGS } else { &["python"] };
                langs
                    .iter()
                    .map(move |lang| (format!("{name}/{lang}"), get_mask(name, lang).unwrap()))
            })
            .collect()
    }

    #[test]
    fn transitions_are_chrome_that_fits_and_names_nothing_forbidden() {
        for (label, m) in all_masks() {
            for width in [3, 40, 60, 120] {
                for chapter in 1..20 {
                    let lines = m.transition(chapter, width, 57);
                    assert!(!lines.is_empty(), "{label}");
                    for l in &lines {
                        let p = l.plain();
                        assert!(!l.is_book && l.pos.is_none(), "{label}: {p}");
                        assert!(
                            unicode_width::UnicodeWidthStr::width(p.as_str()) <= width.max(1),
                            "{label} at {width}: {p}"
                        );
                        let low = p.to_lowercase();
                        assert!(!FORBIDDEN.iter().any(|w| low.contains(w)), "{label}: {p}");
                    }
                }
            }
        }
    }

    #[test]
    fn transitions_speak_each_masks_idiom() {
        let want: &[(&str, &str)] = &[
            ("code", "# --- "),
            ("log", "INFO  svc.worker"),
            ("diff", "diff --git a/"),
            ("test", "tests/test_"),
            ("build", "   Compiling core-"),
            ("git", "Merge branch '"),
            ("docker", "worker-1 exited with code"),
            ("agent", "> "),
        ];
        for (name, marker) in want {
            let m = get_mask(name, "python").unwrap();
            let text: Vec<String> = m.transition(2, 120, 40).iter().map(Line::plain).collect();
            assert!(text.iter().any(|l| l.contains(marker)), "{name}: {text:?}");
        }
    }

    #[test]
    fn transitions_are_stable_vary_by_chapter_and_keep_their_length() {
        for (label, m) in all_masks() {
            let a = m.transition(3, 80, 100);
            assert_eq!(a, m.transition(3, 80, 100), "{label}");
            assert_ne!(a, m.transition(4, 80, 100), "{label}");
            for start in [1, 100, 9_999] {
                assert_eq!(m.transition(3, 80, start).len(), a.len(), "{label}");
            }
        }
    }

    #[test]
    fn seeded_fmt_gives_the_seed_of_the_built_string() {
        let mut a = seeded("bk:(1, 2):code:3:filler");
        let mut b = seeded_fmt(format_args!("bk:({}, {}):{}:{}:filler", 1, 2, "code", 3));
        assert_eq!(random(&mut a), random(&mut b));
        assert_eq!(random(&mut a), random(&mut b));
    }

    #[test]
    fn line_count_matches_render() {
        let paras: Vec<Paragraph> = [1, 7, 40, 200]
            .iter()
            .map(|n| Paragraph::new("kelime ".repeat(*n)))
            .chain([Paragraph::new("x".repeat(300))])
            .collect();
        for name in MASK_NAMES {
            let langs: &[&str] = if *name == "code" { LANGS } else { &["python"] };
            for lang in langs {
                for width in [40, 60, 120] {
                    let mut m = get_mask(name, lang).unwrap();
                    for (seed, p) in paras.iter().enumerate() {
                        let n = m.render(p, (0, 0), width, &mut rng(seed as u64), 1).len();
                        assert_eq!(m.line_count(p, width), n, "{name}/{lang} at {width}");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng as _;

    #[test]
    fn seeded_generators_repeat_and_differ() {
        let a: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 1):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        let b: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 1):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        let c: Vec<u32> = (0..5)
            .map(|_| 0)
            .scan(seeded("bk:(0, 2):code:2"), |r, _| Some(r.r#gen()))
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
