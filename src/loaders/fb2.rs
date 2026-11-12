//! FictionBook 2 (`_fb2_chapters` in `loaders/mupdf.py`): `<section>`s become chapters.

use std::path::Path;

use super::xml::{self, Element};
use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

pub struct Fb2Loader;

/// The document as text, in the encoding its XML declaration names (UTF-8 if none).
fn decode(data: &[u8]) -> String {
    let head = String::from_utf8_lossy(&data[..data.len().min(200)]);
    let label = head
        .split_once("encoding=")
        .and_then(|(_, rest)| {
            let quote = rest.chars().next().filter(|q| matches!(q, '"' | '\''))?;
            rest[1..].split(quote).next()
        })
        .and_then(|l| encoding_rs::Encoding::for_label(l.trim().as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = label.decode(data);
    // the declaration would contradict the now-UTF-8 text
    match text.find("?>") {
        Some(end) if text.trim_start().starts_with("<?xml") => text[end + 2..].to_string(),
        _ => text.into_owned(),
    }
}

fn squash(el: &Element) -> String {
    el.text().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text blocks of one element: paragraphs, verses and subtitles; images as `[asset: name]`.
fn flatten_one(el: &Element, out: &mut Vec<String>) {
    match el.local() {
        "p" | "subtitle" | "v" => out.push(squash(el)),
        "image" => {
            let href = el.attr("href").unwrap_or("");
            let name = href
                .trim_start_matches('#')
                .rsplit('/')
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                out.push(format!("[asset: {name}]"));
            }
        }
        "epigraph" | "cite" | "poem" | "stanza" | "annotation" => {
            for c in el.elements() {
                flatten_one(c, out);
            }
        }
        _ => {}
    }
}

fn emit(chapters: &mut Vec<Chapter>, title: Option<String>, raw: &[String]) {
    let paras = normalize_paragraphs(raw);
    if !paras.is_empty() {
        chapters.push(Chapter {
            title,
            paragraphs: paras.into_iter().map(Paragraph::new).collect(),
        });
    }
}

/// A section's own text is one chapter; text after a nested section starts an untitled one.
fn walk(sec: &Element, chapters: &mut Vec<Chapter>) {
    let mut title = sec
        .elements()
        .find(|c| c.local() == "title")
        .map(squash)
        .filter(|t| !t.is_empty());
    let mut buf = vec![];
    for c in sec.elements() {
        match c.local() {
            "section" => {
                emit(chapters, title.take(), &buf);
                buf.clear();
                walk(c, chapters);
            }
            "title" => {}
            _ => flatten_one(c, &mut buf),
        }
    }
    emit(chapters, title, &buf);
}

pub fn fb2_chapters(data: &[u8]) -> Result<Vec<Chapter>, String> {
    let root = xml::parse(&decode(data))?;
    let mut chapters = vec![];
    for body in root
        .elements()
        .filter(|e| e.local() == "body" && e.attr("name") != Some("notes"))
    {
        let sections: Vec<&Element> = body.elements().filter(|e| e.local() == "section").collect();
        if sections.is_empty() {
            // no sections: the body's paragraphs as one chapter
            let mut buf = vec![];
            for c in body.elements() {
                flatten_one(c, &mut buf);
            }
            emit(&mut chapters, None, &buf);
        }
        for sec in sections {
            walk(sec, &mut chapters);
        }
    }
    Ok(chapters)
}

impl Loader for Fb2Loader {
    fn name(&self) -> &'static str {
        "fb2"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".fb2"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        let start = &head[..head.len().min(1024)];
        has_suffix(path, self.extensions()) || start.windows(12).any(|w| w == b"<FictionBook")
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let chapters = fb2_chapters(&data)
            .map_err(|e| FccError::UnsupportedFormat(format!("dosya açılamadı: {e}")))?;
        Ok(Book {
            id: book_id(&data),
            chapters,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;

    const FB2: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0">
<description><title-info><book-title>Deneme</book-title></title-info></description>
<body>
<section><title><p>Birinci Bölüm</p></title><p>İlk bölümün metni burada.</p></section>
<section><title><p>İkinci Bölüm</p></title><p>İkinci bölümün metni şurada.</p></section>
</body>
</FictionBook>
"#;

    const FB2_RICH: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<FictionBook xmlns="http://www.gribuser.ru/xml/fictionbook/2.0" xmlns:l="http://www.w3.org/1999/xlink">
<body>
<section><title><p>Ana</p></title>
<epigraph><p>Epigraf metni.</p></epigraph>
<p>Giriş.</p>
<subtitle>Alt başlık</subtitle>
<image l:href="#cover.jpg"/>
<poem><stanza><v>Dize bir</v><v>Dize iki</v></stanza></poem>
<cite><p>Alıntı metni.</p></cite>
<section><title><p>İç</p></title><p>İç metin.</p></section>
<p>Son paragraf.</p>
</section>
</body>
<body name="notes"><section><p>Dipnot</p></section></body>
</FictionBook>
"##;

    fn load(name: &str, data: &[u8]) -> Book {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(name);
        std::fs::write(&p, data).unwrap();
        load_book(&p).unwrap()
    }

    fn texts(c: &Chapter) -> Vec<&str> {
        c.paragraphs.iter().map(|p| p.text.as_str()).collect()
    }

    // -- test_mupdf.py (fb2) --

    #[test]
    fn fb2_two_chapters() {
        let book = load("book.fb2", FB2.as_bytes());
        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].title.as_deref(), Some("Birinci Bölüm"));
        assert_eq!(texts(&book.chapters[0]), vec!["İlk bölümün metni burada."]);
        assert_eq!(
            texts(&book.chapters[1]),
            vec!["İkinci bölümün metni şurada."]
        );
    }

    #[test]
    fn fb2_rich_content() {
        let book = load("rich.fb2", FB2_RICH.as_bytes());
        let titles: Vec<Option<&str>> = book.chapters.iter().map(|c| c.title.as_deref()).collect();
        assert_eq!(titles, vec![Some("Ana"), Some("İç"), None]); // the notes body is left out
        assert_eq!(
            texts(&book.chapters[0]),
            vec![
                "Epigraf metni.",
                "Giriş.",
                "Alt başlık",
                "[asset: cover.jpg]",
                "Dize bir",
                "Dize iki",
                "Alıntı metni."
            ]
        );
        assert_eq!(texts(&book.chapters[1]), vec!["İç metin."]);
        assert_eq!(texts(&book.chapters[2]), vec!["Son paragraf."]);
    }

    #[test]
    fn fb2_in_windows_1251_is_decoded() {
        let doc = r#"<?xml version="1.0" encoding="windows-1251"?><FictionBook><body><section><p>Привет, мир.</p></section></body></FictionBook>"#;
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode(doc);
        assert_eq!(
            texts(&load("ru.fb2", &bytes).chapters[0]),
            vec!["Привет, мир."]
        );
    }

    #[test]
    fn fb2_is_recognised_without_its_suffix() {
        let head = b"<?xml version='1.0'?><FictionBook xmlns='x'>";
        assert!(Fb2Loader.can_load(Path::new("noext"), head));
        assert!(!Fb2Loader.can_load(Path::new("a.txt"), b"hello"));
    }
}
