//! Kindle MOBI / AZW / AZW3 (`loaders/azw3.py`, and MOBI via MuPDF in `loaders/mupdf.py`).
//!
//! The `mobi` crate decompresses the text records (PalmDOC or Huffman) into
//! the book's HTML; `<mbp:pagebreak/>` marks chapter breaks in it.

use std::path::Path;

use super::html::html_to_paragraphs;
use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;

pub struct MobiLoader;

/// PalmDB record 0 → the MOBI header's encryption field (0 none, 1/2 DRM); None if unparsable.
pub fn encryption_type(data: &[u8]) -> Option<u16> {
    if data.len() < 82 || &data[60..68] != b"BOOKMOBI" {
        return None;
    }
    let rec0 = u32::from_be_bytes(data[78..82].try_into().ok()?) as usize;
    let field = data.get(rec0 + 12..rec0 + 14)?;
    Some(u16::from_be_bytes([field[0], field[1]]))
}

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

/// The book's HTML, or why it could not be had. The parser is guarded: a
/// malformed file must end in an error, never in a crash.
fn content(data: Vec<u8>) -> Result<String, String> {
    crate::quietly(move || {
        let m = mobi::Mobi::new(&data).map_err(|e| e.to_string())?;
        Ok(m.content_as_string_lossy())
    })
    .unwrap_or_else(|_| Err("bozuk kayıt".into()))
}

pub fn html_chapters(html: &str) -> Vec<Chapter> {
    html.split("<mbp:pagebreak")
        .filter_map(|part| {
            // drop what is left of the cut tag: `/>` or ` />`
            let part = part
                .split_once('>')
                .map(|(head, rest)| if head.len() <= 2 { rest } else { part })
                .unwrap_or(part);
            let (title, paras) = html_to_paragraphs(part);
            (!paras.is_empty()).then(|| Chapter {
                title,
                paragraphs: paras.into_iter().map(Paragraph::new).collect(),
            })
        })
        .collect()
}

impl Loader for MobiLoader {
    fn name(&self) -> &'static str {
        "mobi"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".mobi", ".azw", ".azw3", ".prc"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        has_suffix(path, self.extensions()) || head.get(60..68) == Some(b"BOOKMOBI".as_slice())
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        if encryption_type(&data).is_some_and(|t| t != 0) {
            return Err(FccError::DrmProtected("DRM korumalı, açılamaz".into()));
        }
        let id = book_id(&data);
        let html = content(data).map_err(unsupported)?;
        Ok(Book {
            id,
            chapters: html_chapters(&html),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mobi/sample.azw3")
    }

    // -- test_azw3.py --

    #[test]
    fn azw3_fixture() {
        assert!(MobiLoader.can_load(&fixture(), b""));
        let book = MobiLoader.load(&fixture()).unwrap();
        assert!(book.paragraph_count() > 20, "{}", book.paragraph_count());
        assert!(
            book.chapters
                .iter()
                .flat_map(|c| &c.paragraphs)
                .any(|p| p.text.to_lowercase().contains("wallpaper"))
        );
    }

    #[test]
    fn azw3_encrypted_header_is_drm() {
        let mut data = std::fs::read(fixture()).unwrap();
        let rec0 = u32::from_be_bytes(data[78..82].try_into().unwrap()) as usize;
        assert_eq!(&data[rec0 + 12..rec0 + 14], &[0, 0]);
        data[rec0 + 12..rec0 + 14].copy_from_slice(&2u16.to_be_bytes());
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("drm.azw3");
        std::fs::write(&f, &data).unwrap();
        assert_eq!(
            MobiLoader.load(&f).unwrap_err(),
            FccError::DrmProtected("DRM korumalı, açılamaz".into())
        );
    }

    #[test]
    fn azw3_garbage_is_unsupported_not_drm() {
        use rand::{RngCore, SeedableRng};
        let d = tempfile::tempdir().unwrap();
        let mut junk = vec![0u8; 2048];
        rand_pcg::Pcg32::seed_from_u64(3).fill_bytes(&mut junk);
        let f = d.path().join("junk.azw3");
        std::fs::write(&f, &junk).unwrap();
        assert!(matches!(load_book(&f), Err(FccError::UnsupportedFormat(_))));
    }

    #[test]
    fn page_breaks_split_chapters() {
        let html = "<html><body><h1>Bir</h1><p>alfa</p><mbp:pagebreak/><h2>İki</h2><p>beta</p><mbp:pagebreak /></body></html>";
        let chapters = html_chapters(html);
        let titles: Vec<Option<&str>> = chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Bir"), Some("İki")]);
        assert_eq!(chapters[1].paragraphs[0].text, "beta");
    }
}
