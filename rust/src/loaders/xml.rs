//! A small XML tree for the zip-of-XML formats (DOCX, ODT; later FB2 and EPUB).
//!
//! Names are kept qualified as written (`w:p`, `text:h`): the prefixes these
//! formats use are fixed by convention, and matching on them keeps WordprocessingML
//! paragraphs apart from DrawingML ones that share the local name `p`.

use quick_xml::XmlVersion;
use quick_xml::events::Event;
use quick_xml::reader::Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    /// The name without its prefix.
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    /// An attribute by qualified name (`r:embed`), or by local name when no prefixed one matches.
    pub fn attr(&self, name: &str) -> Option<&str> {
        let exact = self.attrs.iter().find(|(k, _)| k == name);
        let local = || {
            self.attrs
                .iter()
                .find(|(k, _)| k.rsplit(':').next() == Some(name))
        };
        exact.or_else(local).map(|(_, v)| v.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// This element and everything below it, in document order.
    pub fn descendants(&self) -> Vec<&Element> {
        let mut out = vec![self];
        for e in self.elements() {
            out.extend(e.descendants());
        }
        out
    }

    pub fn find(&self, name: &str) -> Option<&Element> {
        self.descendants().into_iter().find(|e| e.name == name)
    }

    /// All text below this element, concatenated.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for c in &self.children {
            match c {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) => out.push_str(&e.text()),
            }
        }
        out
    }
}

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        // XHTML entities that turn up in e-books without a DTD
        "nbsp" => '\u{a0}',
        "shy" => '\u{ad}',
        "mdash" => '—',
        "ndash" => '–',
        "hellip" => '…',
        "laquo" => '«',
        "raquo" => '»',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        _ => return None,
    })
}

fn push_text(parent: &mut Element, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(Node::Text(last)) = parent.children.last_mut() {
        last.push_str(text);
    } else {
        parent.children.push(Node::Text(text.to_string()));
    }
}

/// The document's root element.
pub fn parse(xml: &str) -> Result<Element, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().expand_empty_elements = true;
    let mut stack = vec![Element::default()];
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => stack.push(Element {
                name: e.name().as_ref().to_string(),
                attrs: e
                    .attributes()
                    .filter_map(Result::ok)
                    .map(|a| {
                        let key = a.key.as_ref().to_string();
                        let value = a
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map(|v| v.into_owned())
                            .unwrap_or_default();
                        (key, value)
                    })
                    .collect(),
                children: vec![],
            }),
            Event::End(_) => {
                let done = stack.pop().ok_or("unbalanced document")?;
                stack
                    .last_mut()
                    .ok_or("unbalanced document")?
                    .children
                    .push(Node::Element(done));
            }
            Event::Text(t) => {
                let text = t.xml_content(XmlVersion::Implicit1_0);
                push_text(stack.last_mut().ok_or("unbalanced document")?, &text);
            }
            Event::CData(t) => {
                let text = t.xml_content(XmlVersion::Implicit1_0).into_owned();
                push_text(stack.last_mut().ok_or("unbalanced document")?, &text);
            }
            Event::GeneralRef(r) => {
                let ch = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c),
                    _ => entity(&r.xml_content(XmlVersion::Implicit1_0)),
                };
                if let Some(c) = ch {
                    push_text(
                        stack.last_mut().ok_or("unbalanced document")?,
                        &c.to_string(),
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let root = stack.pop().ok_or("empty document")?;
    if !stack.is_empty() {
        return Err("unclosed element".into());
    }
    root.children
        .into_iter()
        .find_map(|c| match c {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
        .ok_or_else(|| "no root element".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_tree_with_text_entities_and_attributes() {
        let root =
            parse(r#"<?xml version="1.0"?><a:r x="1&amp;2"><b>Ça&amp;&#246;&nbsp;y</b><c/></a:r>"#)
                .unwrap();
        assert_eq!((root.name.as_str(), root.local()), ("a:r", "r"));
        assert_eq!(root.attr("x"), Some("1&2"));
        assert_eq!(root.text(), "Ça&ö\u{a0}y");
        let names: Vec<&str> = root.descendants().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a:r", "b", "c"]);
    }

    #[test]
    fn broken_documents_are_errors() {
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a>").is_err());
        assert!(parse("no xml").is_err());
    }
}
