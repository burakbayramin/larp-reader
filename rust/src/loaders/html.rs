//! HTML pages, and the HTML inside EPUB and MOBI files (`loaders/html.py`).

use std::path::Path;

use ego_tree::NodeRef;
use scraper::{Html, Node};

use super::{Loader, asset_name, decode, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

const SKIP: &[&str] = &["script", "style", "nav", "head"];
const BLOCKS: &[&str] = &[
    "p",
    "div",
    "li",
    "blockquote",
    "ul",
    "ol",
    "section",
    "article",
    "body",
    "html",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "br",
    "tr",
    "pre",
    "table",
];

struct Walk {
    title: Option<String>,
    out: Vec<String>,
    buf: String,
}

impl Walk {
    fn flush(&mut self) {
        if !self.buf.is_empty() {
            self.out
                .extend(normalize_paragraphs(&[std::mem::take(&mut self.buf)]));
        }
    }

    fn walk(&mut self, node: NodeRef<'_, Node>) {
        for child in node.children() {
            match child.value() {
                Node::Text(t) => self.buf.push_str(t),
                Node::Element(el) => {
                    let name = el.name();
                    if SKIP.contains(&name) {
                        continue;
                    }
                    if name == "img" {
                        self.flush();
                        let asset = asset_name(el.attr("src").unwrap_or(""));
                        if !asset.is_empty() {
                            self.out.push(format!("[asset: {asset}]"));
                        }
                    } else if matches!(name, "h1" | "h2") && self.title.is_none() {
                        self.flush();
                        let text = text_of(child, " ");
                        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            self.title = Some(text);
                        }
                    } else if BLOCKS.contains(&name) {
                        self.flush();
                        self.walk(child);
                        self.flush();
                    } else {
                        self.walk(child);
                    }
                }
                _ => {} // comments, doctypes, processing instructions
            }
        }
    }
}

/// All text below `node`, pieces joined with `sep`.
fn text_of(node: NodeRef<'_, Node>, sep: &str) -> String {
    node.descendants()
        .filter_map(|n| n.value().as_text().map(|t| t.to_string()))
        .collect::<Vec<_>>()
        .join(sep)
}

/// (first h1/h2 text or None, paragraphs). Images become `[asset: <basename>]`.
pub fn html_to_paragraphs(html: &str) -> (Option<String>, Vec<String>) {
    let doc = Html::parse_document(html);
    let mut w = Walk {
        title: None,
        out: vec![],
        buf: String::new(),
    };
    w.walk(doc.tree.root());
    w.flush();
    (w.title, w.out)
}

pub struct HtmlLoader;

impl Loader for HtmlLoader {
    fn name(&self) -> &'static str {
        "html"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".html", ".htm", ".xhtml"]
    }

    fn can_load(&self, path: &Path, _head: &[u8]) -> bool {
        has_suffix(path, self.extensions())
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let (title, paras) = html_to_paragraphs(&decode(&data));
        Ok(Book {
            id: book_id(&data),
            chapters: vec![Chapter {
                title,
                paragraphs: paras.into_iter().map(Paragraph::new).collect(),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // -- test_html.py --

    #[test]
    fn html_paragraphs() {
        let html = "<html><head><title>T</title><style>p{}</style></head><body>\
            <nav><p>menü</p></nav><script>var x = 1;</script>\
            <h1>Başlık &amp; Eş</h1><p>Birinci   paragraf &lt;ok&gt;.</p>\
            <ul><li>madde</li></ul><blockquote>alıntı</blockquote>\
            <p><img src='a/b/res.png'></p><!-- yorum --></body></html>";
        let (title, paras) = html_to_paragraphs(html);
        assert_eq!(title.as_deref(), Some("Başlık & Eş"));
        assert_eq!(
            paras,
            vec![
                "Birinci paragraf <ok>.",
                "madde",
                "alıntı",
                "[asset: res.png]"
            ]
        );
    }

    #[test]
    fn html_nested_blocks_not_duplicated() {
        let (_, paras) = html_to_paragraphs("<div><p>x</p></div><div><div>y</div></div>");
        assert_eq!(paras, vec!["x", "y"]);
    }

    #[test]
    fn html_h2_title_and_heading_not_paragraph() {
        let (title, paras) = html_to_paragraphs("<h2>Bölüm</h2><p>metin</p><h2>İkinci</h2>");
        assert_eq!(title.as_deref(), Some("Bölüm"));
        assert_eq!(paras, vec!["metin", "İkinci"]); // only the first heading is the title
    }

    #[test]
    fn html_loose_text_in_div_kept() {
        let (_, paras) = html_to_paragraphs("<div>serbest <b>kalın</b> metin</div>");
        assert_eq!(paras, vec!["serbest kalın metin"]);
    }

    #[test]
    fn html_loader_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.html");
        fs::write(&p, "<h1>Ba</h1><p>İçerik şöyle.</p>").unwrap();
        assert!(HtmlLoader.can_load(&p, b""));
        let book = HtmlLoader.load(&p).unwrap();
        assert_eq!(book.chapters[0].title.as_deref(), Some("Ba"));
        assert_eq!(book.chapters[0].paragraphs[0].text, "İçerik şöyle.");
    }

    #[test]
    fn html_loader_cp1254_fallback() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.htm");
        let (bytes, _, _) = encoding_rs::WINDOWS_1254.encode("<p>ığşçöü</p>");
        fs::write(&p, &bytes).unwrap();
        assert_eq!(
            HtmlLoader.load(&p).unwrap().chapters[0].paragraphs[0].text,
            "ığşçöü"
        );
    }
}
