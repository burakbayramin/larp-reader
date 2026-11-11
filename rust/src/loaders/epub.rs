//! EPUB 2 and 3 (`loaders/epub.py`): the spine's documents, in order, as chapters.
//!
//! Read directly from the zip (container.xml → package document → spine)
//! rather than through an e-book crate: it is a few dozen lines and keeps the
//! DRM check and the HTML handling the same as for every other format.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use super::html::html_to_paragraphs;
use super::xml;
use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;

/// Font obfuscation (IDPF / Adobe) is not DRM.
const FONT_ALGORITHMS: &[&str] = &[
    "http://www.idpf.org/2008/embedding",
    "http://ns.adobe.com/pdf/enc#RC",
];
const DOCUMENT_TYPES: &[&str] = &["application/xhtml+xml", "text/html"];

pub struct EpubLoader;

type Zip = zip::ZipArchive<Cursor<Vec<u8>>>;

fn unsupported(e: impl std::fmt::Display) -> FccError {
    FccError::UnsupportedFormat(format!("dosya açılamadı: {e}"))
}

fn drm() -> FccError {
    FccError::DrmProtected("DRM korumalı, açılamaz".into())
}

fn read_bytes(zip: &mut Zip, name: &str) -> Option<Vec<u8>> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = vec![];
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn read_text(zip: &mut Zip, name: &str) -> Option<String> {
    read_bytes(zip, name).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn check_drm(zip: &mut Zip) -> Result<(), FccError> {
    let Some(raw) = read_text(zip, "META-INF/encryption.xml") else {
        return Ok(());
    };
    let root = xml::parse(&raw).map_err(|_| drm())?;
    let encrypted = root
        .descendants()
        .into_iter()
        .filter(|e| e.local() == "EncryptionMethod")
        .any(|e| !FONT_ALGORITHMS.contains(&e.attr("Algorithm").unwrap_or("")));
    if encrypted { Err(drm()) } else { Ok(()) }
}

/// `dir` + `href` as a zip member name: `..` and `.` resolved, `%20` decoded.
fn resolve(dir: &str, href: &str) -> String {
    let href = super::asset_path(href);
    let mut parts: Vec<&str> = if dir.is_empty() {
        vec![]
    } else {
        dir.split('/').collect()
    };
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

impl Loader for EpubLoader {
    fn name(&self) -> &'static str {
        "epub"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".epub"]
    }

    fn can_load(&self, path: &Path, _head: &[u8]) -> bool {
        has_suffix(path, self.extensions())
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let id = book_id(&data);
        let mut zip = Zip::new(Cursor::new(data)).map_err(unsupported)?;
        check_drm(&mut zip)?;

        let container = read_text(&mut zip, "META-INF/container.xml")
            .ok_or_else(|| unsupported("container.xml yok"))?;
        let opf_path = xml::parse(&container)
            .map_err(unsupported)?
            .descendants()
            .into_iter()
            .find(|e| e.local() == "rootfile")
            .and_then(|e| e.attr("full-path").map(String::from))
            .ok_or_else(|| unsupported("rootfile yok"))?;
        let opf = read_text(&mut zip, &opf_path).ok_or_else(|| unsupported("paket belgesi yok"))?;
        let opf = xml::parse(&opf).map_err(unsupported)?;
        let opf_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

        // id -> (href, media type, properties)
        let manifest: HashMap<&str, (&str, &str, &str)> = opf
            .descendants()
            .into_iter()
            .filter(|e| e.local() == "item")
            .filter_map(|e| {
                Some((
                    e.attr("id")?,
                    (
                        e.attr("href")?,
                        e.attr("media-type").unwrap_or(""),
                        e.attr("properties").unwrap_or(""),
                    ),
                ))
            })
            .collect();
        let spine: Vec<&str> = opf
            .descendants()
            .into_iter()
            .filter(|e| e.local() == "itemref")
            .filter_map(|e| e.attr("idref"))
            .collect();

        let mut chapters = vec![];
        for idref in spine {
            let Some(&(href, media, props)) = manifest.get(idref) else {
                continue;
            };
            if !DOCUMENT_TYPES.contains(&media) || props.split_whitespace().any(|p| p == "nav") {
                continue; // images, styles and the navigation page are not text
            }
            let Some(raw) = read_bytes(&mut zip, &resolve(opf_dir, href)) else {
                continue;
            };
            let (title, paras) = html_to_paragraphs(&String::from_utf8_lossy(&raw));
            if !paras.is_empty() {
                chapters.push(Chapter {
                    title,
                    paragraphs: paras.into_iter().map(Paragraph::new).collect(),
                });
            }
        }
        Ok(Book { id, chapters })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::loaders::load_book;
    use crate::loaders::tests_support::write_zip;
    use std::path::PathBuf;

    const XHTML: &str = r#"<?xml version="1.0" encoding="utf-8"?><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml"><head><title>t</title></head><body>"#;

    /// The Python test's book: nav page, two chapters with an empty one between, an image.
    pub fn make_epub(path: &Path, encryption: Option<&str>) -> PathBuf {
        let opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
            <metadata/><manifest>
              <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
              <item id="c1" href="text/c1.xhtml" media-type="application/xhtml+xml"/>
              <item id="empty" href="text/empty.xhtml" media-type="application/xhtml+xml"/>
              <item id="c2" href="text/Ikinci%20Bolum.xhtml" media-type="application/xhtml+xml"/>
              <item id="img" href="images/kapak.png" media-type="image/png"/>
            </manifest><spine><itemref idref="nav"/><itemref idref="c1"/><itemref idref="empty"/><itemref idref="c2"/></spine></package>"#;
        let nav = format!(
            "{XHTML}<nav><ol><li>Birinci</li><li>İkinci</li></ol></nav><p>gezinme</p></body></html>"
        );
        let c1 = format!(
            "{XHTML}<h1>Birinci Bölüm</h1><p>İlk bölümün metni: ığşçöü.</p><p><img src='../images/kapak.png'/></p></body></html>"
        );
        let empty = format!("{XHTML}<div>  </div></body></html>");
        let c2 = format!(
            "{XHTML}<h2>İkinci Bölüm</h2><p>İkinci bölümün metni şurada.</p></body></html>"
        );
        let container = r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
            <rootfiles><rootfile full-path="EPUB/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
        let mut files = vec![
            ("mimetype", "application/epub+zip"),
            ("META-INF/container.xml", container),
            ("EPUB/content.opf", opf),
            ("EPUB/nav.xhtml", nav.as_str()),
            ("EPUB/text/c1.xhtml", c1.as_str()),
            ("EPUB/text/empty.xhtml", empty.as_str()),
            ("EPUB/text/Ikinci Bolum.xhtml", c2.as_str()),
            ("EPUB/images/kapak.png", "png"),
        ];
        if let Some(enc) = encryption {
            files.push(("META-INF/encryption.xml", enc));
        }
        write_zip(path, &files);
        path.to_path_buf()
    }

    fn encryption(algorithm: &str, uri: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"
               xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="{algorithm}"/>
               <enc:CipherData><enc:CipherReference URI="{uri}"/></enc:CipherData></enc:EncryptedData></encryption>"#
        )
    }

    // -- test_epub.py --

    #[test]
    fn epub_spine_order_and_titles() {
        let d = tempfile::tempdir().unwrap();
        let book = EpubLoader
            .load(&make_epub(&d.path().join("book.epub"), None))
            .unwrap();
        // the nav page is skipped and the empty document dropped
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Birinci Bölüm"), Some("İkinci Bölüm")]);
        assert_eq!(
            book.chapters[0].paragraphs[0].text,
            "İlk bölümün metni: ığşçöü."
        );
        assert_eq!(
            book.chapters[1].paragraphs[0].text,
            "İkinci bölümün metni şurada."
        );
    }

    #[test]
    fn epub_image_becomes_asset_line() {
        let d = tempfile::tempdir().unwrap();
        let book = EpubLoader
            .load(&make_epub(&d.path().join("book.epub"), None))
            .unwrap();
        assert!(
            book.chapters[0]
                .paragraphs
                .iter()
                .any(|p| p.text == "[asset: kapak.png]")
        );
    }

    #[test]
    fn epub_goes_through_load_book() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            load_book(&make_epub(&d.path().join("book.epub"), None))
                .unwrap()
                .chapters
                .len(),
            2
        );
    }

    #[test]
    fn epub_with_encryption_xml_raises_drm() {
        let d = tempfile::tempdir().unwrap();
        let enc = encryption("http://www.w3.org/2001/04/xmlenc#aes128-cbc", "c1.xhtml");
        let p = make_epub(&d.path().join("drm.epub"), Some(&enc));
        assert_eq!(
            EpubLoader.load(&p).unwrap_err(),
            FccError::DrmProtected("DRM korumalı, açılamaz".into())
        );
        assert!(matches!(load_book(&p), Err(FccError::DrmProtected(_))));
    }

    #[test]
    fn epub_font_obfuscation_is_not_drm() {
        let d = tempfile::tempdir().unwrap();
        for algorithm in FONT_ALGORITHMS {
            let enc = encryption(algorithm, "fonts/a.otf");
            let p = make_epub(&d.path().join("font.epub"), Some(&enc));
            assert!(EpubLoader.load(&p).unwrap().paragraph_count() >= 3);
        }
    }

    #[test]
    fn hrefs_resolve_against_the_package_folder() {
        assert_eq!(resolve("OEBPS", "text/a.xhtml"), "OEBPS/text/a.xhtml");
        assert_eq!(
            resolve("OEBPS/text", "../images/b%20c.png"),
            "OEBPS/images/b c.png"
        );
        assert_eq!(resolve("", "./c.xhtml#frag"), "c.xhtml");
    }
}
