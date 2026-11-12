//! Word documents (`loaders/docx.py`): `word/document.xml` inside a zip.
//!
//! Heading 1 and 2 paragraphs start chapters; images become `[asset: name]`.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use super::xml::{self, Element};
use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;

pub struct DocxLoader;

type Zip = zip::ZipArchive<Cursor<Vec<u8>>>;

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

/// A member of the zip as text; None if it is not there.
pub(super) fn member(zip: &mut Zip, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// "Heading 1", "heading2", "Başlık 1" … in either the style's name or its id.
fn is_heading_style(name: &str) -> bool {
    let low = name.to_lowercase();
    ["heading", "başlık", "baslik"].iter().any(|w| {
        low.strip_prefix(w)
            .is_some_and(|rest| matches!(rest.trim_start(), "1" | "2"))
    })
}

/// What python-docx's `Paragraph.text` gives: run text, tabs and breaks.
fn paragraph_text(p: &Element) -> String {
    let mut out = String::new();
    for e in p.descendants() {
        match e.name.as_str() {
            "w:t" => out.push_str(&e.text()),
            "w:tab" => out.push('\t'),
            "w:br" | "w:cr" => out.push('\n'),
            _ => {}
        }
    }
    out
}

impl Loader for DocxLoader {
    fn name(&self) -> &'static str {
        "docx"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".docx"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        has_suffix(path, self.extensions()) && head.starts_with(b"PK")
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let id = book_id(&data);
        let mut zip = Zip::new(Cursor::new(data)).map_err(unsupported)?;
        let body_xml = member(&mut zip, "word/document.xml")
            .ok_or_else(|| unsupported("word/document.xml yok"))?;
        let doc = xml::parse(&body_xml).map_err(unsupported)?;

        let styles: HashMap<String, String> = member(&mut zip, "word/styles.xml")
            .and_then(|s| xml::parse(&s).ok())
            .map(|root| {
                root.elements()
                    .filter(|e| e.name == "w:style")
                    .filter_map(|s| {
                        let id = s.attr("w:styleId")?.to_string();
                        let name = s
                            .elements()
                            .find(|e| e.name == "w:name")?
                            .attr("w:val")?
                            .to_string();
                        Some((id, name))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let rels: HashMap<String, String> = member(&mut zip, "word/_rels/document.xml.rels")
            .and_then(|s| xml::parse(&s).ok())
            .map(|root| {
                root.elements()
                    .filter_map(|r| {
                        Some((r.attr("Id")?.to_string(), r.attr("Target")?.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let body = doc
            .find("w:body")
            .ok_or_else(|| unsupported("w:body yok"))?;
        let mut chapters = vec![];
        let mut title: Option<String> = None;
        let mut paras: Vec<Paragraph> = vec![];
        for p in body.elements().filter(|e| e.name == "w:p") {
            let text = paragraph_text(p)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let style_id = p
                .elements()
                .find(|e| e.name == "w:pPr")
                .and_then(|ppr| ppr.elements().find(|e| e.name == "w:pStyle"))
                .and_then(|s| s.attr("w:val"));
            let heading = style_id.is_some_and(|id| {
                is_heading_style(id) || styles.get(id).is_some_and(|name| is_heading_style(name))
            });
            if !text.is_empty() && heading {
                if !paras.is_empty() {
                    chapters.push(Chapter {
                        title: title.take(),
                        paragraphs: std::mem::take(&mut paras),
                    });
                }
                title = Some(text);
                continue;
            }
            if !text.is_empty() {
                paras.push(Paragraph::new(text));
            }
            for blip in p.descendants().into_iter().filter(|e| e.name == "a:blip") {
                if let Some(target) = blip.attr("r:embed").and_then(|rid| rels.get(rid)) {
                    let name = target.rsplit('/').next().unwrap_or(target);
                    paras.push(Paragraph::new(format!("[asset: {name}]")));
                }
            }
        }
        if !paras.is_empty() {
            chapters.push(Chapter {
                title,
                paragraphs: paras,
            });
        }
        Ok(Book { id, chapters })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;
    use crate::loaders::tests_support::write_zip;

    const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

    fn para(text: &str) -> String {
        format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
    }

    fn heading(text: &str, level: u8) -> String {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading{level}"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#
        )
    }

    fn docx(dir: &Path, body: &str, extra: &[(&str, &str)]) -> std::path::PathBuf {
        let document = format!(
            r#"<?xml version="1.0"?><w:document {W}><w:body>{body}<w:sectPr/></w:body></w:document>"#
        );
        let styles = format!(
            r#"<w:styles {W}><w:style w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>
               <w:style w:styleId="Heading2"><w:name w:val="heading 2"/></w:style></w:styles>"#
        );
        let mut files = vec![
            ("word/document.xml", document.as_str()),
            ("word/styles.xml", styles.as_str()),
        ];
        files.extend_from_slice(extra);
        let p = dir.join("b.docx");
        write_zip(&p, &files);
        p
    }

    fn texts(book: &Book) -> Vec<Vec<String>> {
        book.chapters
            .iter()
            .map(|c| c.paragraphs.iter().map(|p| p.text.clone()).collect())
            .collect()
    }

    // -- test_office.py (docx) --

    #[test]
    fn docx_headings_make_chapters() {
        let d = tempfile::tempdir().unwrap();
        let body = [
            para("preface text"),
            heading("One", 1),
            para("alpha"),
            para("beta"),
            heading("Two", 2),
            para("gamma"),
        ]
        .concat();
        let p = docx(d.path(), &body, &[]);
        assert!(DocxLoader.can_load(&p, &std::fs::read(&p).unwrap()[..4]));
        let book = load_book(&p).unwrap();
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![None, Some("One"), Some("Two")]);
        assert_eq!(
            texts(&book),
            vec![vec!["preface text"], vec!["alpha", "beta"], vec!["gamma"]]
        );
    }

    #[test]
    fn docx_no_untitled_chapter_when_heading_first() {
        let d = tempfile::tempdir().unwrap();
        let p = docx(d.path(), &[heading("One", 1), para("alpha")].concat(), &[]);
        let titles: Vec<Option<String>> = load_book(&p)
            .unwrap()
            .chapters
            .into_iter()
            .map(|c| c.title)
            .collect();
        assert_eq!(titles, vec![Some("One".to_string())]);
    }

    #[test]
    fn docx_inline_image_becomes_asset() {
        let d = tempfile::tempdir().unwrap();
        let picture = r#"<w:p xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:r><w:drawing><a:graphic><a:graphicData><a:blip r:embed="rId9"/></a:graphicData></a:graphic></w:drawing></w:r></w:p>"#;
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
            <Relationship Id="rId9" Target="media/image1.png"/></Relationships>"#;
        let p = docx(
            d.path(),
            &[para("before"), picture.to_string(), para("after")].concat(),
            &[("word/_rels/document.xml.rels", rels)],
        );
        assert_eq!(
            texts(&load_book(&p).unwrap()),
            vec![vec!["before", "[asset: image1.png]", "after"]]
        );
    }

    #[test]
    fn docx_tables_are_skipped_and_runs_join() {
        let d = tempfile::tempdir().unwrap();
        let body = r#"<w:p><w:r><w:t>Bir</w:t></w:r><w:r><w:t xml:space="preserve"> iki</w:t><w:tab/><w:t>üç</w:t></w:r></w:p>
            <w:tbl><w:tr><w:tc><w:p><w:r><w:t>hücre</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
        let p = docx(d.path(), body, &[]);
        assert_eq!(texts(&load_book(&p).unwrap()), vec![vec!["Bir iki üç"]]);
    }

    #[test]
    fn heading_style_names() {
        for yes in ["Heading 1", "heading2", "Başlık 1", "BASLIK 2", "Heading1"] {
            assert!(is_heading_style(yes), "{yes}");
        }
        for no in ["Heading 3", "Normal", "Title", "Heading"] {
            assert!(!is_heading_style(no), "{no}");
        }
    }
}
