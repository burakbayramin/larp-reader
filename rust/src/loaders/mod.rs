//! Turning a file into a `Book` (`loaders/__init__.py`).
//!
//! Loaders are tried in order; the first that claims the file and loads it
//! wins. DRM and missing-text errors stop the search (another loader will not
//! do better), any other failure lets the next loader try.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::book::Book;
use crate::errors::FccError;

pub mod docx;
pub mod epub;
pub mod fb2;
pub mod html;
pub mod mobi;
pub mod odt;
pub mod rtf;
pub mod text;
pub mod xml;

pub trait Loader {
    fn name(&self) -> &'static str;
    /// Lower-case suffixes, with the dot.
    fn extensions(&self) -> &'static [&'static str];
    /// Whether to try this file; `head` is its first 4 KiB.
    fn can_load(&self, path: &Path, head: &[u8]) -> bool;
    fn load(&self, path: &Path) -> Result<Book, FccError>;
}

/// In the order they are tried.
pub fn loaders() -> Vec<Box<dyn Loader>> {
    vec![
        Box::new(text::TextLoader),
        Box::new(epub::EpubLoader),
        Box::new(html::HtmlLoader),
        Box::new(mobi::MobiLoader),
        Box::new(docx::DocxLoader),
        Box::new(odt::OdtLoader),
        Box::new(rtf::RtfLoader),
        Box::new(fb2::Fb2Loader),
    ]
}

/// Lower-case suffixes some loader opens directly (without a converter).
pub fn supported_extensions() -> Vec<&'static str> {
    loaders()
        .iter()
        .flat_map(|l| l.extensions().iter().copied())
        .collect()
}

/// The file's suffix, lower-cased, with the dot ("" if none).
pub fn suffix(path: &Path) -> String {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_default()
}

pub fn has_suffix(path: &Path, extensions: &[&str]) -> bool {
    extensions.contains(&suffix(path).as_str())
}

/// Text in UTF-8 (with or without BOM), else Windows-1254, the usual Turkish legacy encoding.
pub fn decode(data: &[u8]) -> String {
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1254
            .decode_without_bom_handling(data)
            .0
            .into_owned(),
    }
}

/// An asset reference without fragment or query, decoded: `a/b%20c.png?v=1#f` → `a/b c.png`.
pub fn asset_path(src: &str) -> String {
    let src = src.split('#').next().unwrap_or("");
    percent_decode(src.split('?').next().unwrap_or(""))
}

/// The last part of an asset reference, decoded: `a/b/res%20x.png?v=1#f` → `res x.png`.
pub fn asset_name(src: &str) -> String {
    asset_path(src)
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A program on PATH, like `shutil.which`.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or(".EXE;.BAT;.CMD".into())
            .split(';')
            .map(String::from)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{name}{ext}")))
            .find(|p| p.is_file())
    })
}

pub fn load_book(path: &Path) -> Result<Book, FccError> {
    let mut head = Vec::with_capacity(4096);
    File::open(path)
        .and_then(|f| f.take(4096).read_to_end(&mut head))
        .map_err(|e| FccError::from_io(&e))?;
    for loader in loaders() {
        if !loader.can_load(path, &head) {
            continue;
        }
        match loader.load(path) {
            Ok(book) if book.paragraph_count() == 0 => {
                return Err(FccError::EmptyBook(
                    "dosyada okunabilir metin bulunamadı".into(),
                ));
            }
            Ok(book) => return Ok(book),
            Err(e @ (FccError::DrmProtected(_) | FccError::NoTextLayer(_))) => return Err(e),
            Err(_) => continue, // the next loader may still manage
        }
    }
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or("(uzantısız)".into());
    // the suffix only, never the file name: it may be the title
    let mut msg = format!("\"{ext}\" biçimi açılamadı");
    if find_program("ebook-convert").is_none() {
        msg.push_str("; Calibre (ebook-convert) kurarak tekrar deneyin");
    }
    Err(FccError::UnsupportedFormat(msg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn asset_names_lose_paths_queries_and_escapes() {
        assert_eq!(asset_name("a/b/res%20x.png?v=1#f"), "res x.png");
        assert_eq!(asset_name("images/"), "images");
        assert_eq!(asset_name(""), "");
        assert_eq!(asset_name("resim%Ç.png"), "resim%Ç.png"); // not an escape, and no panic
        assert_eq!(asset_name("%C3%A7ay.png"), "çay.png");
    }

    #[test]
    fn decode_reads_utf8_with_bom_and_falls_back_to_turkish_windows() {
        assert_eq!(decode("\u{feff}Çay".as_bytes()), "Çay");
        let (cp1254, _, _) = encoding_rs::WINDOWS_1254.encode("Şişli'de ğüıöç");
        assert_eq!(decode(&cp1254), "Şişli'de ğüıöç");
    }

    #[test]
    fn unknown_extension_is_unsupported_and_never_names_the_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("Gizli Başlık.xyz");
        fs::write(&p, b"\x00\x01binary").unwrap();
        let e = load_book(&p).unwrap_err();
        assert!(matches!(e, FccError::UnsupportedFormat(_)));
        assert!(e.to_string().starts_with("\".xyz\" biçimi açılamadı"));
        assert!(!e.to_string().contains("Gizli"));
    }

    #[test]
    fn missing_file_is_input_unavailable_without_its_name() {
        let d = tempfile::tempdir().unwrap();
        let e = load_book(&d.path().join("Suç ve Ceza.txt")).unwrap_err();
        assert_eq!(e, FccError::InputUnavailable("input file not found".into()));
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use std::io::Write;
    use std::path::Path;

    /// A zip file holding these (name, text) members.
    pub fn write_zip(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, text) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
}

#[cfg(test)]
mod chain_tests {
    use super::*;
    use rand::{RngCore, SeedableRng};

    #[test]
    fn corrupt_office_files_are_unsupported() {
        let d = tempfile::tempdir().unwrap();
        let mut rng = rand_pcg::Pcg32::seed_from_u64(7);
        for ext in ["docx", "odt", "rtf"] {
            let mut junk = vec![0u8; 2048];
            rng.fill_bytes(&mut junk);
            let p = d.path().join(format!("bad.{ext}"));
            std::fs::write(&p, &junk).unwrap();
            assert!(
                matches!(load_book(&p), Err(FccError::UnsupportedFormat(_))),
                "{ext}"
            );
        }
    }

    #[test]
    fn a_zip_that_only_looks_like_docx_is_unsupported() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("fake.docx");
        tests_support::write_zip(&p, &[("word/other.xml", "<a/>")]);
        assert!(matches!(load_book(&p), Err(FccError::UnsupportedFormat(_))));
    }
}
