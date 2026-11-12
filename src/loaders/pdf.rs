//! PDF (`loaders/mupdf.py`), through the pure-Rust `pdf_oxide`.
//!
//! Text lines are grouped into blocks by the gaps between them, images sit
//! among the blocks by position, and the rest follows the Python version:
//! - a block that opens or closes at least half the pages (digits ignored) is a
//!   running header or footer and is dropped, once there are 3 pages or more;
//! - a word split by a soft hyphen at a line end is joined;
//! - outline entries split the text into chapters;
//! - no text at all means a scan: "no text layer".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use pdf_oxide::document::PdfDocument;
use pdf_oxide::outline::{Destination, OutlineItem};

use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

const MIN_PAGES_FOR_HEADER_STRIP: usize = 3;
/// Lines further apart than this share of a line's height start a new block.
const BLOCK_GAP: f32 = 0.5;

pub struct PdfLoader;

/// What is on a page, top to bottom.
#[derive(Debug, Clone, PartialEq)]
enum Block {
    Text(String),
    Image,
}

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

/// The text with whitespace squashed and digits blanked: page numbers do not make a footer differ.
fn mask(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect()
}

fn page_blocks(doc: &PdfDocument, page: usize) -> Result<Vec<Block>, FccError> {
    let read_err =
        |e: pdf_oxide::error::Error| FccError::UnsupportedFormat(format!("dosya okunamadı: {e}"));
    let mut lines = doc.extract_text_lines(page).map_err(read_err)?;
    // PDF space: y grows upward, so the top of the page comes first in descending y
    lines.sort_by(|a, b| (b.bbox.y + b.bbox.height).total_cmp(&(a.bbox.y + a.bbox.height)));

    // (top, block) so images can be placed among the text
    let mut placed: Vec<(f32, Block)> = vec![];
    let mut cur: Vec<String> = vec![];
    let mut cur_top = 0.0;
    let mut prev: Option<(f32, f32)> = None; // (bottom, height) of the previous line
    for line in &lines {
        let (top, bottom, h) = (
            line.bbox.y + line.bbox.height,
            line.bbox.y,
            line.bbox.height,
        );
        if let Some((prev_bottom, prev_h)) = prev
            && prev_bottom - top > BLOCK_GAP * prev_h.max(h)
        {
            placed.push((cur_top, Block::Text(cur.join("\n"))));
            cur.clear();
        }
        if cur.is_empty() {
            cur_top = top;
        }
        cur.push(line.text.clone());
        prev = Some((bottom, h));
    }
    if !cur.is_empty() {
        placed.push((cur_top, Block::Text(cur.join("\n"))));
    }
    for img in doc.extract_images(page).unwrap_or_default() {
        let top = img.bbox().map(|r| r.y + r.height).unwrap_or(f32::MIN); // unknown position: last
        placed.push((top, Block::Image));
    }
    placed.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(placed.into_iter().map(|(_, b)| b).collect())
}

fn text_blocks(blocks: &[Block]) -> Vec<&str> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::Text(t) if !t.trim().is_empty() => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

/// Masked texts that open or close at least half the pages.
fn repeated_keys(pages: &[Vec<Block>]) -> HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for blocks in pages {
        let texts = text_blocks(blocks);
        let (Some(first), Some(last)) = (texts.first(), texts.last()) else {
            continue;
        };
        let keys: HashSet<String> = [mask(first), mask(last)].into();
        for k in keys {
            *counts.entry(k).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| n * 2 >= pages.len())
        .map(|(k, _)| k)
        .collect()
}

/// A page's paragraphs and how many of them are real text (not image markers).
fn page_paragraphs(
    blocks: &[Block],
    page_no: usize,
    drop: &HashSet<String>,
) -> (Vec<String>, usize) {
    let texts = text_blocks(blocks);
    let (first, last) = (texts.first().copied(), texts.last().copied());
    let mut out = vec![];
    let mut text_count = 0;
    let mut img_no = 0;
    for b in blocks {
        match b {
            Block::Image => {
                img_no += 1;
                out.push(format!("[asset: page{page_no}_img{img_no}]"));
            }
            Block::Text(t) if !t.trim().is_empty() => {
                let edge = Some(t.as_str()) == first || Some(t.as_str()) == last;
                if edge && drop.contains(&mask(t)) {
                    continue;
                }
                let norm = normalize_paragraphs(&[t.replace("\u{ad}\n", "")]);
                text_count += norm.len();
                out.extend(norm);
            }
            Block::Text(_) => {}
        }
    }
    (out, text_count)
}

/// (page index, title) of every outline entry that points at a page, in outline order.
fn outline_starts(items: &[OutlineItem], out: &mut Vec<(usize, String)>) {
    for it in items {
        if let Some(Destination::PageIndex(p)) = it.dest {
            out.push((p, it.title.trim().to_string()));
        }
        outline_starts(&it.children, out);
    }
}

impl Loader for PdfLoader {
    fn name(&self) -> &'static str {
        "pdf"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".pdf"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        has_suffix(path, self.extensions()) || head.starts_with(b"%PDF")
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let id = book_id(&data);
        let doc = PdfDocument::from_bytes(data).map_err(unsupported)?;
        if doc.is_encrypted() && !doc.is_authenticated() {
            return Err(FccError::DrmProtected("DRM korumalı, açılamaz".into()));
        }
        let n_pages = doc.page_count().map_err(unsupported)?;
        let pages = (0..n_pages)
            .map(|i| page_blocks(&doc, i))
            .collect::<Result<Vec<_>, _>>()?;

        let drop = if pages.len() >= MIN_PAGES_FOR_HEADER_STRIP {
            repeated_keys(&pages)
        } else {
            HashSet::new()
        };
        let mut per_page = vec![];
        let mut total_text = 0;
        for (i, blocks) in pages.iter().enumerate() {
            let (paras, n) = page_paragraphs(blocks, i + 1, &drop);
            per_page.push(paras);
            total_text += n;
        }
        if total_text == 0 {
            return Err(FccError::NoTextLayer(
                "Metin katmanı yok, OCR desteklenmiyor".into(),
            ));
        }

        let chapter = |title: Option<String>, start: usize, end: usize| Chapter {
            title,
            paragraphs: per_page[start..end]
                .iter()
                .flatten()
                .cloned()
                .map(Paragraph::new)
                .collect(),
        };
        let mut entries = vec![];
        if let Ok(Some(items)) = doc.get_outline() {
            outline_starts(&items, &mut entries);
        }
        // the first entry for a page names its chapter
        let mut starts: BTreeMap<usize, Option<String>> = BTreeMap::new();
        for (page, title) in entries {
            let page = page.min(n_pages.saturating_sub(1));
            starts
                .entry(page)
                .or_insert((!title.is_empty()).then_some(title));
        }
        let mut chapters = vec![];
        if starts.is_empty() {
            chapters.push(chapter(None, 0, n_pages));
        } else {
            let bounds: Vec<usize> = starts.keys().copied().collect();
            if bounds[0] > 0 {
                chapters.push(chapter(None, 0, bounds[0]));
            }
            for (k, &s) in bounds.iter().enumerate() {
                let e = bounds.get(k + 1).copied().unwrap_or(n_pages);
                chapters.push(chapter(starts[&s].clone(), s, e));
            }
        }
        chapters.retain(|c| !c.paragraphs.is_empty());
        Ok(Book { id, chapters })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pdf")
            .join(name)
    }

    fn texts(book: &Book) -> Vec<String> {
        book.chapters
            .iter()
            .flat_map(|c| c.paragraphs.iter().map(|p| p.text.clone()))
            .collect()
    }

    // -- test_mupdf.py (pdf) --

    #[test]
    fn pdf_paragraphs_turkish_text() {
        let path = fixture("three_pages.pdf");
        let book = PdfLoader.load(&path).unwrap();
        assert!(texts(&book).contains(&"Türkçe karakterler: ığşçöü İstanbul.".to_string()));
        assert_eq!(book.id, book_id(&std::fs::read(&path).unwrap()));
    }

    #[test]
    fn pdf_footer_removed() {
        let t = texts(&PdfLoader.load(&fixture("three_pages.pdf")).unwrap());
        assert!(!t.iter().any(|x| x.starts_with("Sayfa")), "{t:?}");
        assert!(t.iter().any(|x| x.contains("Ağır ağır")));
    }

    #[test]
    fn pdf_footer_kept_when_not_repeated() {
        // with only 2 pages header/footer stripping must not run
        let t = texts(&PdfLoader.load(&fixture("two_pages.pdf")).unwrap());
        assert!(
            t.contains(&"Sayfa 1".to_string()) && t.contains(&"Sayfa 2".to_string()),
            "{t:?}"
        );
    }

    #[test]
    fn one_page_pdf_single_block_survives() {
        assert_eq!(
            texts(&PdfLoader.load(&fixture("one_page.pdf")).unwrap()),
            vec!["Tek blok metin."]
        );
    }

    #[test]
    fn pdf_hyphen_joined() {
        let t = texts(&PdfLoader.load(&fixture("three_pages.pdf")).unwrap());
        assert!(t.contains(&"bölümlendirme tamam.".to_string()), "{t:?}");
    }

    #[test]
    fn encrypted_pdf_raises_drm() {
        assert!(matches!(
            PdfLoader.load(&fixture("encrypted.pdf")),
            Err(FccError::DrmProtected(_))
        ));
    }

    #[test]
    fn image_only_pdf_raises_no_text_layer() {
        assert!(matches!(
            PdfLoader.load(&fixture("image_only.pdf")),
            Err(FccError::NoTextLayer(_))
        ));
    }

    #[test]
    fn load_book_routes_pdf_here() {
        assert!(
            load_book(&fixture("three_pages.pdf"))
                .unwrap()
                .paragraph_count()
                > 0
        );
    }

    #[test]
    fn corrupt_pdf_is_unsupported_format() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad.pdf");
        std::fs::write(&p, (0..=255u8).cycle().take(2048).collect::<Vec<u8>>()).unwrap();
        assert!(matches!(load_book(&p), Err(FccError::UnsupportedFormat(_))));
    }

    #[test]
    fn can_load_magic_and_extension() {
        assert!(PdfLoader.can_load(Path::new("a.pdf"), b""));
        assert!(PdfLoader.can_load(Path::new("noext"), b"%PDF-1.7"));
        assert!(!PdfLoader.can_load(Path::new("a.txt"), b"hello"));
    }

    #[test]
    fn pdf_image_block_becomes_asset() {
        assert_eq!(
            texts(&PdfLoader.load(&fixture("text_image.pdf")).unwrap()),
            vec!["Resimli sayfa metni.", "[asset: page1_img1]"]
        );
    }

    #[test]
    fn pdf_toc_splits_chapters() {
        let book = PdfLoader.load(&fixture("toc.pdf")).unwrap();
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Bir"), Some("Iki")]);
        let per: Vec<Vec<&str>> = book
            .chapters
            .iter()
            .map(|c| c.paragraphs.iter().map(|p| p.text.as_str()).collect())
            .collect();
        assert_eq!(
            per,
            vec![
                vec!["Sayfa metni alfa.", "Sayfa metni beta."],
                vec!["Sayfa metni gama.", "Sayfa metni delta."]
            ]
        );
    }
}
