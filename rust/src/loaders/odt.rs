//! OpenDocument text (`loaders/odt.py`): `content.xml` inside a zip.
//!
//! `text:h` headings start chapters; images become `[asset: name]`.

use std::io::{Cursor, Read};
use std::path::Path;

use super::docx::member;
use super::xml::{self, Element, Node};
use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;

const MIMETYPE: &str = "application/vnd.oasis.opendocument.text";
const CONTAINERS: &[&str] = &["text:list", "text:list-item", "text:section"];

pub struct OdtLoader;

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

/// Text as odfpy's `teletype.extractText` gives it: spaces, tabs and line breaks expanded.
fn extract_text(el: &Element) -> String {
    let mut out = String::new();
    for c in &el.children {
        match c {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => match e.name.as_str() {
                "text:s" => {
                    let n = e
                        .attr("text:c")
                        .and_then(|c| c.parse::<usize>().ok())
                        .unwrap_or(1);
                    out.push_str(&" ".repeat(n));
                }
                "text:tab" => out.push('\t'),
                "text:line-break" => out.push('\n'),
                _ => out.push_str(&extract_text(e)),
            },
        }
    }
    out
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Walk {
    chapters: Vec<Chapter>,
    title: Option<String>,
    paras: Vec<Paragraph>,
}

impl Walk {
    fn flush(&mut self) {
        if !self.paras.is_empty() {
            self.chapters.push(Chapter {
                title: self.title.clone(),
                paragraphs: std::mem::take(&mut self.paras),
            });
        }
    }

    fn walk(&mut self, node: &Element) {
        for child in node.elements() {
            match child.name.as_str() {
                "text:h" => {
                    let text = squash(&extract_text(child));
                    if !text.is_empty() {
                        self.flush();
                        self.title = Some(text);
                    }
                }
                "text:p" => {
                    let text = squash(&extract_text(child));
                    if !text.is_empty() {
                        self.paras.push(Paragraph::new(text));
                    }
                    for img in child
                        .descendants()
                        .into_iter()
                        .filter(|e| e.name == "draw:image")
                    {
                        let href = img.attr("xlink:href").unwrap_or("");
                        let name = href.rsplit('/').next().unwrap_or("");
                        if !name.is_empty() {
                            self.paras.push(Paragraph::new(format!("[asset: {name}]")));
                        }
                    }
                }
                name if CONTAINERS.contains(&name) => self.walk(child),
                _ => {} // tables, frames outside paragraphs, …
            }
        }
    }
}

impl Loader for OdtLoader {
    fn name(&self) -> &'static str {
        "odt"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".odt"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        if !has_suffix(path, self.extensions()) || !head.starts_with(b"PK") {
            return false;
        }
        let Ok(data) = std::fs::read(path) else {
            return false;
        };
        let Ok(mut zip) = zip::ZipArchive::new(Cursor::new(data)) else {
            return false;
        };
        let Ok(mut f) = zip.by_name("mimetype") else {
            return false;
        };
        let mut kind = String::new();
        f.read_to_string(&mut kind).is_ok() && kind.trim() == MIMETYPE
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let id = book_id(&data);
        let mut zip = zip::ZipArchive::new(Cursor::new(data)).map_err(unsupported)?;
        let content =
            member(&mut zip, "content.xml").ok_or_else(|| unsupported("content.xml yok"))?;
        let doc = xml::parse(&content).map_err(unsupported)?;
        let text = doc
            .find("office:text")
            .ok_or_else(|| unsupported("office:text yok"))?;
        let mut w = Walk {
            chapters: vec![],
            title: None,
            paras: vec![],
        };
        w.walk(text);
        w.flush();
        Ok(Book {
            id,
            chapters: w.chapters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;
    use crate::loaders::tests_support::write_zip;

    fn odt(dir: &Path, body: &str) -> std::path::PathBuf {
        let content = format!(
            r#"<?xml version="1.0"?><office:document-content
                xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
                xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
                xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
                xmlns:xlink="http://www.w3.org/1999/xlink">
               <office:body><office:text>{body}</office:text></office:body></office:document-content>"#
        );
        let p = dir.join("b.odt");
        write_zip(&p, &[("mimetype", MIMETYPE), ("content.xml", &content)]);
        p
    }

    fn texts(book: &Book) -> Vec<Vec<String>> {
        book.chapters
            .iter()
            .map(|c| c.paragraphs.iter().map(|p| p.text.clone()).collect())
            .collect()
    }

    // -- test_office.py (odt) --

    #[test]
    fn odt_headings_make_chapters() {
        let d = tempfile::tempdir().unwrap();
        let p = odt(
            d.path(),
            r#"<text:p>preface text</text:p><text:h text:outline-level="1">One</text:h><text:p>alpha</text:p>
               <text:h text:outline-level="2">Two</text:h><text:p>beta</text:p>"#,
        );
        assert!(OdtLoader.can_load(&p, b"PK"));
        let book = load_book(&p).unwrap();
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![None, Some("One"), Some("Two")]);
        assert_eq!(
            texts(&book),
            vec![vec!["preface text"], vec!["alpha"], vec!["beta"]]
        );
    }

    #[test]
    fn odt_image_becomes_asset() {
        let d = tempfile::tempdir().unwrap();
        let p = odt(
            d.path(),
            r#"<text:p>before</text:p><text:p><draw:frame><draw:image xlink:href="Pictures/abc123.png"/></draw:frame></text:p>
               <text:p>after</text:p>"#,
        );
        assert_eq!(
            texts(&load_book(&p).unwrap()),
            vec![vec!["before", "[asset: abc123.png]", "after"]]
        );
    }

    #[test]
    fn odt_spaces_lists_and_spans() {
        let d = tempfile::tempdir().unwrap();
        let p = odt(
            d.path(),
            r#"<text:list><text:list-item><text:p>madde <text:span>bir</text:span></text:p></text:list-item></text:list>
               <text:p>a<text:s text:c="3"/>b<text:tab/>c</text:p>"#,
        );
        assert_eq!(
            texts(&load_book(&p).unwrap()),
            vec![vec!["madde bir", "a b c"]]
        );
    }

    #[test]
    fn odt_rejects_wrong_mimetype() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.odt");
        write_zip(&p, &[("mimetype", "application/epub+zip")]);
        assert!(!OdtLoader.can_load(&p, b"PK"));
    }
}
