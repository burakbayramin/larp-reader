//! Rich Text Format (`loaders/rtf.py`).
//!
//! A small RTF-to-text reader in the spirit of `striprtf`: it keeps the text,
//! turns `\par` into line breaks, decodes `\'hh` with the document's code page
//! and `\uN` as Unicode, and drops destinations that are not text (font and
//! colour tables, pictures, document info, `\*` groups …).

use std::path::Path;

use encoding_rs::Encoding;

use super::{Loader, has_suffix};
use crate::book::{Book, Chapter, Paragraph, book_id};
use crate::errors::FccError;
use crate::text::normalize_paragraphs;

pub struct RtfLoader;

/// Groups whose content is not part of the text.
const DESTINATIONS: &[&str] = &[
    "aftncn",
    "aftnsep",
    "aftnsepc",
    "annotation",
    "atnauthor",
    "atndate",
    "atnicn",
    "atnid",
    "atnparent",
    "atnref",
    "atntime",
    "atrfend",
    "atrfstart",
    "author",
    "background",
    "bkmkend",
    "bkmkstart",
    "blipuid",
    "buptim",
    "category",
    "colorschememapping",
    "colortbl",
    "comment",
    "company",
    "creatim",
    "datafield",
    "datastore",
    "defchp",
    "defpap",
    "do",
    "doccomm",
    "docvar",
    "dptxbxtext",
    "factoidname",
    "falt",
    "fchars",
    "ffdeftext",
    "ffentrymcr",
    "ffexitmcr",
    "ffformat",
    "ffhelptext",
    "ffl",
    "ffname",
    "ffstattext",
    "file",
    "filetbl",
    "fldinst",
    "fldtype",
    "fname",
    "fontemb",
    "fontfile",
    "fonttbl",
    "footer",
    "footerf",
    "footerl",
    "footerr",
    "footnote",
    "formfield",
    "ftncn",
    "ftnsep",
    "ftnsepc",
    "g",
    "generator",
    "gridtbl",
    "header",
    "headerf",
    "headerl",
    "headerr",
    "hl",
    "hlfr",
    "hlinkbase",
    "hlloc",
    "hlsrc",
    "hsv",
    "htmltag",
    "info",
    "keycode",
    "keywords",
    "latentstyles",
    "lchars",
    "levelnumbers",
    "leveltext",
    "lfolevel",
    "linkval",
    "list",
    "listlevel",
    "listname",
    "listoverride",
    "listoverridetable",
    "listpicture",
    "liststylename",
    "listtable",
    "listtext",
    "lsdlockedexcept",
    "macc",
    "maccPr",
    "mailmerge",
    "manager",
    "mmath",
    "mmathPict",
    "mmathPr",
    "nesttableprops",
    "nextfile",
    "nonesttables",
    "objalias",
    "objclass",
    "objdata",
    "object",
    "objname",
    "objsect",
    "objtime",
    "oldcprops",
    "oldpprops",
    "oldsprops",
    "oldtprops",
    "oleclsid",
    "operator",
    "panose",
    "password",
    "passwordhash",
    "pgp",
    "pgptbl",
    "picprop",
    "pict",
    "pn",
    "pnseclvl",
    "pntext",
    "pntxta",
    "pntxtb",
    "printim",
    "private",
    "propname",
    "protend",
    "protstart",
    "protusertbl",
    "pxe",
    "result",
    "revtbl",
    "revtim",
    "rsidtbl",
    "rxe",
    "shp",
    "shpgrp",
    "shpinst",
    "shppict",
    "shprslt",
    "shptxt",
    "sn",
    "sp",
    "staticval",
    "stylesheet",
    "subject",
    "sv",
    "svb",
    "tc",
    "template",
    "themedata",
    "title",
    "txe",
    "ud",
    "upr",
    "userprops",
    "wgrffmtfilter",
    "windowcaption",
    "writereservation",
    "writereservhash",
    "xe",
    "xform",
    "xmlattrname",
    "xmlattrvalue",
    "xmlclose",
    "xmlname",
    "xmlnstbl",
    "xmlopen",
];

fn symbol(word: &str) -> Option<&'static str> {
    Some(match word {
        "par" | "line" | "row" => "\n",
        "sect" | "page" => "\n\n",
        "tab" => "\t",
        "cell" => " ",
        "emdash" => "—",
        "endash" => "–",
        "emspace" | "enspace" | "qmspace" => " ",
        "bullet" => "•",
        "lquote" => "‘",
        "rquote" => "’",
        "ldblquote" => "“",
        "rdblquote" => "”",
        _ => return None,
    })
}

/// The encoding named by `\ansicpgN` in the header (Windows-1252 if none).
fn codepage(head: &[u8]) -> &'static Encoding {
    let text = String::from_utf8_lossy(head);
    let n: Option<String> = text.find("\\ansicpg").map(|at| {
        text[at + 8..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect()
    });
    match n.as_deref() {
        Some("65001") => encoding_rs::UTF_8,
        Some(n) if !n.is_empty() => Encoding::for_label(format!("windows-{n}").as_bytes())
            .or_else(|| Encoding::for_label(format!("cp{n}").as_bytes()))
            .unwrap_or(encoding_rs::WINDOWS_1252),
        _ => encoding_rs::WINDOWS_1252,
    }
}

#[derive(Clone, Copy)]
struct Group {
    ignore: bool,
    /// Fallback characters that follow each `\uN`.
    uc: usize,
}

struct Text {
    enc: &'static Encoding,
    out: String,
    pending: Vec<u8>, // code-page bytes not yet decoded (so multi-byte code pages work)
    skip: usize,      // fallback characters still to drop after a `\uN`
}

impl Text {
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.out
                .push_str(&self.enc.decode_without_bom_handling(&self.pending).0);
            self.pending.clear();
        }
    }

    fn byte(&mut self, b: u8, g: Group) {
        if self.skip > 0 {
            self.skip -= 1;
        } else if !g.ignore {
            self.pending.push(b);
        }
    }

    fn push(&mut self, s: &str, g: Group) {
        if !g.ignore {
            self.flush();
            self.out.push_str(s);
        }
    }
}

pub fn rtf_to_text(data: &[u8]) -> String {
    let mut t = Text {
        enc: codepage(&data[..data.len().min(4096)]),
        out: String::new(),
        pending: vec![],
        skip: 0,
    };
    let mut stack: Vec<Group> = vec![];
    let mut g = Group {
        ignore: false,
        uc: 1,
    };
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'{' => {
                stack.push(g);
                i += 1;
            }
            b'}' => {
                t.flush();
                g = stack.pop().unwrap_or(g);
                i += 1;
            }
            b'\r' | b'\n' => i += 1,
            b'\\' => {
                let Some(&next) = data.get(i + 1) else { break };
                match next {
                    b'\\' | b'{' | b'}' => {
                        t.byte(next, g);
                        i += 2;
                    }
                    b'\'' => {
                        let hex = data.get(i + 2..i + 4).and_then(|h| {
                            u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
                        });
                        if let Some(b) = hex {
                            t.byte(b, g);
                        }
                        i += 4;
                    }
                    b'*' => {
                        g.ignore = true;
                        i += 2;
                    }
                    b'~' => {
                        t.push(" ", g);
                        i += 2;
                    }
                    b'_' => {
                        t.push("-", g);
                        i += 2;
                    }
                    b'\r' | b'\n' => {
                        t.push("\n", g);
                        i += 2;
                    }
                    c if c.is_ascii_alphabetic() => {
                        let start = i + 1;
                        let mut j = start;
                        while j < data.len() && data[j].is_ascii_alphabetic() {
                            j += 1;
                        }
                        let word = std::str::from_utf8(&data[start..j]).unwrap_or("");
                        let num_start = j;
                        if j < data.len() && data[j] == b'-' {
                            j += 1;
                        }
                        while j < data.len() && data[j].is_ascii_digit() {
                            j += 1;
                        }
                        let param: Option<i64> = std::str::from_utf8(&data[num_start..j])
                            .ok()
                            .and_then(|s| s.parse().ok());
                        if j < data.len() && data[j] == b' ' {
                            j += 1; // the delimiting space belongs to the control word
                        }
                        i = j;
                        if DESTINATIONS.contains(&word) {
                            g.ignore = true;
                        } else if word == "uc" {
                            g.uc = param.unwrap_or(1).max(0) as usize;
                        } else if word == "u" {
                            let n = param.unwrap_or(0);
                            let code = if n < 0 { n + 65536 } else { n } as u32;
                            if let Some(ch) = char::from_u32(code) {
                                t.push(&ch.to_string(), g);
                            }
                            t.skip = g.uc;
                        } else if let Some(s) = symbol(word) {
                            t.push(s, g);
                        }
                    }
                    _ => i += 2, // other control symbols (\-, \|, \: …) carry no text
                }
            }
            b => {
                t.byte(b, g);
                i += 1;
            }
        }
    }
    t.flush();
    t.out
}

impl Loader for RtfLoader {
    fn name(&self) -> &'static str {
        "rtf"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[".rtf"]
    }

    fn can_load(&self, path: &Path, head: &[u8]) -> bool {
        has_suffix(path, self.extensions()) && head.trim_ascii_start().starts_with(b"{\\rtf")
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        let text = rtf_to_text(&data);
        let lines: Vec<&str> = text.split('\n').collect();
        let paragraphs = normalize_paragraphs(&lines)
            .into_iter()
            .map(Paragraph::new)
            .collect();
        Ok(Book {
            id: book_id(&data),
            chapters: vec![Chapter {
                title: None,
                paragraphs,
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::load_book;

    const RTF: &str = "{\\rtf1\\ansi\\ansicpg1254\\deff0{\\fonttbl{\\f0 Arial;}}\\f0 \
        Merhaba d\\'fcnya: \\'fe\\'f0\\'fd \\'dd\\'de\\'d0 \\'e7\\'f6\\'fc\\par\r\n\
        Ikinci paragraf.\\par}";

    // -- test_office.py (rtf) --

    #[test]
    fn rtf_turkish_chars() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.rtf");
        std::fs::write(&p, RTF).unwrap();
        assert!(RtfLoader.can_load(&p, RTF.as_bytes()));
        let book = load_book(&p).unwrap();
        let texts: Vec<&str> = book.chapters[0]
            .paragraphs
            .iter()
            .map(|x| x.text.as_str())
            .collect();
        assert_eq!(
            texts,
            vec!["Merhaba dünya: şğı İŞĞ çöü", "Ikinci paragraf."]
        );
    }

    #[test]
    fn unicode_escapes_skip_their_fallback() {
        assert_eq!(
            rtf_to_text(b"{\\rtf1\\uc1 \\u351?ey \\u-3913?}"),
            "şey \u{f0b7}"
        );
    }

    #[test]
    fn starred_groups_and_info_are_dropped() {
        let text = rtf_to_text(
            b"{\\rtf1{\\info{\\title Gizli}}{\\*\\generator Word;}Metin\\tab sonu\\par}",
        );
        assert_eq!(text, "Metin\tsonu\n");
    }

    #[test]
    fn escaped_braces_and_backslashes_are_text() {
        assert_eq!(rtf_to_text(b"{\\rtf1 a\\{b\\}c\\\\d}"), "a{b}c\\d");
    }
}
