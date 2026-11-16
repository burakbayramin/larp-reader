//! `larpr tick`: one status-bar line per call, in the shape of CI or git output.
//!
//! Each call prints the next stretch of the saved place's text after a rotating
//! status prefix, then moves the place forward by what it showed, so a status bar
//! that polls the command reads through the text. The sub-paragraph offset lives
//! in its own `tick.json`; the shared progress file only ever gets whole
//! paragraphs, exactly as the full-screen view writes them.
//! With nothing to show the line is a plain "ok" status and the exit code is 0.

use std::path::Path;

use serde_json::{Value, json};
use unicode_width::UnicodeWidthStr;

use crate::book::Book;
use crate::loaders::load_book;
use crate::state::{Entry, last_entry, load_config, load_progress, save_progress};
use crate::text::truncate_cells;
use crate::ui::app::timestamp;

/// The default `--width`.
pub const DEFAULT_WIDTH: usize = 80;
/// Shown when there is nothing to carry.
const NEUTRAL: &str = "ci: all checks passed";
/// Status prefixes, rotated per call; the text follows after a separator.
const PREFIXES: &[&str] = &[
    "build ok 3.2s",
    "main 2 ahead, 0 behind",
    "ci #4821 passing",
    "tests 212 passed",
    "lint clean",
    "deploy ok",
    "git: 3 modified",
    "cargo check ok",
];
/// A prefix is dropped when the text would get less room than this.
const MIN_TEXT: usize = 12;

/// Where the status line's text left off.
#[derive(Debug, Default, PartialEq, Eq)]
struct Cursor {
    id: String,
    chapter: usize,
    paragraph: usize,
    /// Characters of the paragraph already shown.
    offset: usize,
    /// Calls so far, for rotating the prefix.
    n: usize,
}

fn read_cursor(dir: &Path) -> Cursor {
    let Some(v) = std::fs::read(dir.join("tick.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return Cursor::default();
    };
    let num = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
    Cursor {
        id: v
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        chapter: num("chapter"),
        paragraph: num("paragraph"),
        offset: num("offset"),
        n: num("n"),
    }
}

fn write_cursor(dir: &Path, c: &Cursor) {
    let value = json!({
        "id": c.id, "chapter": c.chapter, "paragraph": c.paragraph,
        "offset": c.offset, "n": c.n,
    });
    let _ = std::fs::write(dir.join("tick.json"), value.to_string());
}

/// The longest run of whole words of `text` (from char `offset`) that fits `cells`, and the
/// new offset. A first word wider than the room is cut. None at the end of the text.
fn take_words(text: &str, offset: usize, cells: usize) -> Option<(String, usize)> {
    let rest: Vec<char> = text.chars().skip(offset).collect();
    let mut i = 0;
    while i < rest.len() && rest[i].is_whitespace() {
        i += 1;
    }
    if i == rest.len() {
        return None;
    }
    let mut out = String::new();
    let mut end = i;
    while end < rest.len() {
        let start = end;
        let mut stop = start;
        while stop < rest.len() && !rest[stop].is_whitespace() {
            stop += 1;
        }
        let word: String = rest[start..stop].iter().collect();
        let sep = usize::from(!out.is_empty());
        if out.width() + sep + word.width() > cells {
            if out.is_empty() {
                let cut = truncate_cells(&word, cells);
                let n = cut.chars().count().max(1);
                return Some((cut, offset + start + n));
            }
            break;
        }
        if sep == 1 {
            out.push(' ');
        }
        out.push_str(&word);
        end = stop;
        while end < rest.len() && rest[end].is_whitespace() {
            end += 1;
        }
    }
    Some((out, offset + end))
}

/// The line for call number `n`: prefix, separator, text, all within `width` cells.
fn compose(text: &str, n: usize, width: usize) -> String {
    let prefix = PREFIXES[n % PREFIXES.len()];
    let head = format!("{prefix} | ");
    if width < head.width() + MIN_TEXT {
        return truncate_cells(text, width);
    }
    format!("{head}{text}")
}

/// Step the cursor over `book`: the line to print and the place after it.
fn step(book: &Book, cur: &mut Cursor, width: usize) -> Option<String> {
    let width = width.max(1);
    let head = PREFIXES[cur.n % PREFIXES.len()].width() + 3;
    let room = if width < head + MIN_TEXT {
        width
    } else {
        width - head
    };
    let mut pos = (cur.chapter, cur.paragraph);
    book.paragraph(pos)?;
    loop {
        let text = &book.paragraph(pos)?.text;
        if let Some((chunk, next)) = take_words(text, cur.offset, room) {
            cur.offset = next;
            let line = compose(&chunk, cur.n, width);
            cur.n += 1;
            return Some(line);
        }
        pos = book.next_pos(pos)?;
        cur.chapter = pos.0;
        cur.paragraph = pos.1;
        cur.offset = 0;
    }
}

/// The status line for this call. Every failure collapses into the neutral line.
pub fn tick(dir: &Path, width: usize) -> String {
    run(dir, width).unwrap_or_else(|| truncate_cells(NEUTRAL, width.max(1)))
}

fn run(dir: &Path, width: usize) -> Option<String> {
    let mut progress = load_progress(dir);
    let path = last_entry(&progress)?.1.path.clone();
    let book = load_book(Path::new(&path)).ok()?;
    let local = progress.get(&book.id)?.clone();
    let sync_dir = load_config(dir).sync_dir;
    let entry = crate::sync::merge(&sync_dir, &book.id, Some(local.clone()))?;

    let mut cur = read_cursor(dir);
    if cur.id != book.id || (cur.chapter, cur.paragraph) != (entry.chapter, entry.paragraph) {
        // the place moved elsewhere (or first call): start at its paragraph's head
        cur = Cursor {
            id: book.id.clone(),
            chapter: entry.chapter,
            paragraph: entry.paragraph,
            offset: 0,
            n: cur.n,
        };
    }
    let line = step(&book, &mut cur, width);
    let (chapter, paragraph) = (cur.chapter, cur.paragraph);
    write_cursor(dir, &cur);
    let moved = (chapter, paragraph) != (entry.chapter, entry.paragraph);
    if moved || entry != local {
        let updated_at = if moved {
            timestamp()
        } else {
            entry.updated_at.clone()
        };
        let saved = Entry {
            chapter,
            paragraph,
            updated_at,
            ..entry
        };
        crate::sync::push(&sync_dir, &book.id, &saved);
        progress.insert(book.id.clone(), saved);
        let _ = save_progress(dir, &progress);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Chapter, Paragraph};
    use std::fs;

    fn book(paras: &[&str]) -> Book {
        Book {
            id: "x".into(),
            chapters: vec![Chapter {
                title: None,
                paragraphs: paras.iter().map(|p| Paragraph::new(*p)).collect(),
            }],
        }
    }

    fn forbidden(text: &str) -> bool {
        let low = text.to_lowercase();
        ["book", "reader", "okuyucu"]
            .iter()
            .any(|w| low.contains(w))
    }

    #[test]
    fn take_words_fits_and_resumes() {
        let t = "one two three four";
        let (a, off) = take_words(t, 0, 8).unwrap();
        assert_eq!(a, "one two");
        let (b, off) = take_words(t, off, 8).unwrap();
        assert_eq!(b, "three");
        let (c, off) = take_words(t, off, 8).unwrap();
        assert_eq!(c, "four");
        assert!(take_words(t, off, 8).is_none());
    }

    #[test]
    fn long_word_is_cut_not_stuck() {
        let (a, off) = take_words("abcdefghij k", 0, 4).unwrap();
        assert_eq!((a.as_str(), off), ("abcd", 4));
    }

    #[test]
    fn step_reads_through_the_text_and_fits() {
        let b = book(&["alpha beta gamma delta", "epsilon zeta"]);
        let mut cur = Cursor::default();
        let mut shown = Vec::new();
        while let Some(line) = step(&b, &mut cur, 30) {
            assert!(line.width() <= 30, "{line}");
            assert!(!forbidden(&line));
            shown.push(line);
            assert!(shown.len() < 50);
        }
        let all = shown.join(" ");
        for w in ["alpha", "delta", "epsilon", "zeta"] {
            assert!(all.contains(w), "{all}");
        }
    }

    #[test]
    fn narrow_width_drops_the_prefix() {
        assert_eq!(compose("hello", 0, 8), "hello");
    }

    fn setup(text: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.txt");
        fs::write(&f, text).unwrap();
        let book = load_book(&f).unwrap();
        let mut p = crate::state::Progress::new();
        p.insert(
            book.id,
            Entry {
                path: f.to_string_lossy().into(),
                chapter: 0,
                paragraph: 0,
                bookmarks: vec![],
                updated_at: "2026-01-01T00:00:00".into(),
            },
        );
        let state = d.path().join("state");
        fs::create_dir(&state).unwrap();
        save_progress(&state, &p).unwrap();
        d
    }

    #[test]
    fn no_book_prints_the_neutral_line() {
        let d = tempfile::tempdir().unwrap();
        let line = tick(d.path(), 80);
        assert_eq!(line, NEUTRAL);
        assert!(!forbidden(&line));
    }

    #[test]
    fn missing_file_prints_the_neutral_line() {
        let d = setup("hello world");
        let state = d.path().join("state");
        fs::remove_file(d.path().join("a.txt")).unwrap();
        assert_eq!(tick(&state, 80), NEUTRAL);
    }

    #[test]
    fn repeated_calls_advance_and_save_the_place() {
        let text = "first paragraph words here and there.\n\nsecond paragraph goes on and on.";
        let d = setup(text);
        let state = d.path().join("state");
        let mut seen = String::new();
        for _ in 0..12 {
            let line = tick(&state, 30);
            assert!(line.width() <= 30, "{line}");
            assert!(!line.contains("a.txt") && !forbidden(&line), "{line}");
            seen.push_str(&line);
            seen.push(' ');
        }
        assert!(seen.contains("first"), "{seen}");
        assert!(seen.contains("second"), "{seen}");
        let p = load_progress(&state);
        assert_eq!(p.values().next().unwrap().paragraph, 1);
    }

    #[test]
    fn tick_syncs_the_place_through_the_shared_folder() {
        let d = setup("aaa bbb\n\nccc ddd\n\neee fff");
        let state = d.path().join("state");
        let shared = d.path().join("shared");
        let cfg = crate::state::Config {
            sync_dir: shared.to_string_lossy().into(),
            ..Default::default()
        };
        crate::state::save_config(&state, &cfg).unwrap();
        for _ in 0..4 {
            tick(&state, 80);
        }
        let id = load_progress(&state).keys().next().unwrap().clone();
        assert!(shared.join(format!("{id}.json")).is_file());
        // a second machine with the same book at another path picks the place up
        let other = d.path().join("other");
        fs::create_dir(&other).unwrap();
        fs::copy(d.path().join("a.txt"), other.join("b.txt")).unwrap();
        let mut p = crate::state::Progress::new();
        p.insert(
            id.clone(),
            Entry {
                path: other.join("b.txt").to_string_lossy().into(),
                chapter: 0,
                paragraph: 0,
                bookmarks: vec![],
                updated_at: "2026-01-01T00:00:00".into(),
            },
        );
        save_progress(&other, &p).unwrap();
        crate::state::save_config(&other, &cfg).unwrap();
        let line = tick(&other, 80);
        assert!(line.contains("eee") || line.contains("ccc"), "{line}");
        assert!(load_progress(&other)[&id].paragraph >= 1);
    }

    #[test]
    fn a_moved_place_restarts_at_its_paragraph() {
        let d = setup("aaa bbb\n\nccc ddd");
        let state = d.path().join("state");
        tick(&state, 80);
        let mut p = load_progress(&state);
        p.values_mut().next().unwrap().paragraph = 1;
        save_progress(&state, &p).unwrap();
        assert!(tick(&state, 80).contains("ccc"));
    }
}
