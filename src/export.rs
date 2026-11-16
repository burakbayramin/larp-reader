//! `larpr export`: the saved marks of one text as Markdown.
//!
//! This is plain command line output for the user, not a disguised screen,
//! so it may name the file and quote its text. Nothing here is drawn by the app.

use std::fmt::Write as _;
use std::path::Path;

use crate::book::{Book, Pos};
use crate::errors::FccError;
use crate::loaders::load_book;
use crate::state::{Entry, load_progress};

/// Load `file`, find its saved entry in `state_dir` and render its marks.
pub fn export(file: &Path, state_dir: &Path) -> Result<String, FccError> {
    let book = load_book(file)?;
    let progress = load_progress(state_dir);
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_string_lossy().into_owned());
    Ok(markdown(&book, progress.get(&book.id), &name))
}

/// The marks in reading order, grouped by chapter, each with its paragraph quoted.
///
/// Marks that no longer point into the text (the file changed) are left out.
pub fn markdown(book: &Book, entry: Option<&Entry>, name: &str) -> String {
    let mut marks: Vec<Pos> = entry
        .map(|e| e.bookmarks.iter().map(|m| (m[0], m[1])).collect())
        .unwrap_or_default();
    marks.retain(|p| book.paragraph(*p).is_some());
    marks.sort_unstable();
    marks.dedup();

    let mut out = format!("# {name}\n\n");
    if let Some(e) = entry.filter(|e| book.paragraph((e.chapter, e.paragraph)).is_some()) {
        let _ = writeln!(
            out,
            "Last position: chapter {}, paragraph {}.\n",
            e.chapter + 1,
            e.paragraph + 1
        );
    }
    if marks.is_empty() {
        out.push_str("No bookmarks yet.\n");
        return out;
    }
    let _ = writeln!(
        out,
        "{} bookmark{}.",
        marks.len(),
        if marks.len() == 1 { "" } else { "s" }
    );

    let mut chapter = None;
    for (ci, pi) in marks {
        if chapter != Some(ci) {
            chapter = Some(ci);
            let title = book.chapters[ci]
                .title
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(String::from)
                .unwrap_or_else(|| format!("Chapter {}", ci + 1));
            let _ = write!(out, "\n## {title}\n");
        }
        let text = &book.chapters[ci].paragraphs[pi].text;
        out.push('\n');
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                out.push_str(">\n");
            } else {
                let _ = writeln!(out, "> {line}");
            }
        }
        let _ = writeln!(out, "\n*Chapter {}, paragraph {}*", ci + 1, pi + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{Chapter, Paragraph};
    use crate::state::{Progress, save_progress};

    fn book() -> Book {
        let ch = |title: Option<&str>, ps: &[&str]| Chapter {
            title: title.map(String::from),
            paragraphs: ps.iter().map(|p| Paragraph::new(*p)).collect(),
        };
        Book {
            id: "abc".into(),
            chapters: vec![
                ch(Some("Opening"), &["first", "second\nline two"]),
                ch(None, &["third", "fourth"]),
            ],
        }
    }

    fn entry(marks: Vec<[usize; 2]>) -> Entry {
        Entry {
            path: "x".into(),
            chapter: 1,
            paragraph: 0,
            bookmarks: marks,
            updated_at: String::new(),
        }
    }

    #[test]
    fn marks_are_sorted_grouped_and_quoted() {
        let md = markdown(
            &book(),
            Some(&entry(vec![[1, 1], [0, 1], [9, 9], [0, 1]])),
            "novel.epub",
        );
        assert_eq!(
            md,
            "# novel.epub\n\n\
             Last position: chapter 2, paragraph 1.\n\n\
             2 bookmarks.\n\
             \n## Opening\n\n> second\n> line two\n\n*Chapter 1, paragraph 2*\n\
             \n## Chapter 2\n\n> fourth\n\n*Chapter 2, paragraph 2*\n"
        );
    }

    #[test]
    fn no_entry_or_no_marks_says_so() {
        assert_eq!(
            markdown(&book(), None, "a.txt"),
            "# a.txt\n\nNo bookmarks yet.\n"
        );
        let md = markdown(&book(), Some(&entry(vec![])), "a.txt");
        assert!(md.ends_with("No bookmarks yet.\n"), "{md}");
        assert!(md.contains("Last position: chapter 2, paragraph 1."));
    }

    #[test]
    fn export_reads_the_saved_marks_of_the_file() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("notes.txt");
        std::fs::write(&f, "Alpha paragraph.\n\nBeta paragraph.\n").unwrap();
        let id = load_book(&f).unwrap().id;
        let mut data = Progress::new();
        let mut e = entry(vec![[0, 1]]);
        e.chapter = 0;
        data.insert(id, e);
        save_progress(d.path(), &data).unwrap();
        let md = export(&f, d.path()).unwrap();
        assert!(md.starts_with("# notes.txt\n"), "{md}");
        assert!(md.contains("> Beta paragraph."), "{md}");
        assert!(!md.contains("Alpha"), "{md}");
    }

    #[test]
    fn export_of_a_missing_file_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        assert!(export(&d.path().join("nope.txt"), d.path()).is_err());
    }
}
