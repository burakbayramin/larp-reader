//! Plain text and Markdown (`loaders/text.py`).

use std::path::Path;

use super::{Loader, decode, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

pub struct TextLoader;

/// Paragraphs: runs of lines split at blank (whitespace-only) lines.
fn chapter(title: Option<String>, lines: &[&str]) -> Chapter {
    let mut blocks: Vec<String> = vec![];
    let mut cur: Vec<&str> = vec![];
    for line in lines {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                blocks.push(cur.join("\n"));
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        blocks.push(cur.join("\n"));
    }
    Chapter {
        title,
        paragraphs: normalize_paragraphs(&blocks)
            .into_iter()
            .map(Paragraph::new)
            .collect(),
    }
}

/// `# Title` or `## Title` (not `###`): a chapter starts.
fn heading(line: &str) -> Option<String> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    let rest = &line[hashes..];
    let title = rest.trim();
    (matches!(hashes, 1 | 2) && rest.starts_with(char::is_whitespace) && !title.is_empty())
        .then(|| title.to_string())
}

impl Loader for TextLoader {
    fn name(&self) -> &'static str {
        "text"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".txt", ".md", ".markdown"]
    }

    fn can_load(&self, path: &Path, _head: &[u8]) -> bool {
        has_suffix(path, self.extensions())
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let text = decode(&data).replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = text.split('\n').collect();
        let chapters = if has_suffix(path, &[".txt"]) {
            vec![chapter(None, &lines)]
        } else {
            let mut chapters = vec![];
            let mut title = None;
            let mut buf = vec![];
            for line in lines {
                if let Some(t) = heading(line) {
                    chapters.push(chapter(title.take(), &buf));
                    title = Some(t);
                    buf.clear();
                } else {
                    buf.push(line);
                }
            }
            chapters.push(chapter(title, &buf));
            // drop an empty preamble (text before the first heading)
            chapters.retain(|c| !c.paragraphs.is_empty() || c.title.is_some());
            chapters
        };
        Ok(Book {
            id: book_id(&data),
            chapters,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::FccError;
    use crate::loaders::load_book;
    use std::fs;

    fn file(name: &str, data: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(name);
        fs::write(&p, data).unwrap();
        (d, p)
    }

    // -- test_text_loader.py --

    #[test]
    fn utf8_txt() {
        let (_d, p) = file(
            "a.txt",
            "Birinci paragraf\nikinci satır.\n\n\nİkinci paragraf.".as_bytes(),
        );
        let b = load_book(&p).unwrap();
        assert_eq!(b.chapters.len(), 1);
        assert_eq!(b.chapters[0].title, None);
        let texts: Vec<&str> = b.chapters[0]
            .paragraphs
            .iter()
            .map(|x| x.text.as_str())
            .collect();
        assert_eq!(
            texts,
            vec!["Birinci paragraf ikinci satır.", "İkinci paragraf."]
        );
        assert_eq!(b.id.len(), 16);
    }

    #[test]
    fn cp1254_txt_turkish() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1254.encode("Şişli'de ğüıöç");
        let (_d, p) = file("a.txt", &bytes);
        assert_eq!(
            load_book(&p).unwrap().paragraph((0, 0)).unwrap().text,
            "Şişli'de ğüıöç"
        );
    }

    #[test]
    fn md_headings_start_chapters() {
        let (_d, p) = file(
            "a.md",
            "# Bir\n\nmetin\n\n# İki\n\nmetin2\n\n### alt başlık değil".as_bytes(),
        );
        let b = load_book(&p).unwrap();
        let titles: Vec<Option<&str>> = b.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Bir"), Some("İki")]);
        assert_eq!(b.chapters[1].paragraphs[0].text, "metin2");
        assert_eq!(b.chapters[1].paragraphs[1].text, "### alt başlık değil");
    }

    #[test]
    fn empty_file_raises_empty_book() {
        let (_d, p) = file("a.txt", b"  \n\n");
        assert!(matches!(load_book(&p), Err(FccError::EmptyBook(_))));
    }

    #[test]
    fn windows_line_endings_and_hyphenation() {
        let (_d, p) = file("a.txt", b"Raskol-\r\nnikov uyandi.\r\n\r\nOda dar.");
        let b = load_book(&p).unwrap();
        let texts: Vec<&str> = b.chapters[0]
            .paragraphs
            .iter()
            .map(|x| x.text.as_str())
            .collect();
        assert_eq!(texts, vec!["Raskolnikov uyandi.", "Oda dar."]);
    }
}
