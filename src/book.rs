//! The text model: a book is chapters of paragraphs (`book.py`).

use sha2::{Digest, Sha256};

/// (chapter index, paragraph index).
pub type Pos = (usize, usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    pub text: String,
}

impl Paragraph {
    pub fn new(text: impl Into<String>) -> Self {
        Paragraph { text: text.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub title: Option<String>,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Book {
    pub id: String,
    pub chapters: Vec<Chapter>,
}

impl Book {
    pub fn paragraph_count(&self) -> usize {
        self.chapters.iter().map(|c| c.paragraphs.len()).sum()
    }

    pub fn paragraph(&self, pos: Pos) -> Option<&Paragraph> {
        self.chapters.get(pos.0)?.paragraphs.get(pos.1)
    }

    pub fn next_pos(&self, (ci, pi): Pos) -> Option<Pos> {
        if pi + 1 < self.chapters[ci].paragraphs.len() {
            return Some((ci, pi + 1));
        }
        (ci + 1..self.chapters.len())
            .find(|&c| !self.chapters[c].paragraphs.is_empty())
            .map(|c| (c, 0))
    }

    pub fn prev_pos(&self, (ci, pi): Pos) -> Option<Pos> {
        if pi > 0 {
            return Some((ci, pi - 1));
        }
        (0..ci)
            .rev()
            .find(|&c| !self.chapters[c].paragraphs.is_empty())
            .map(|c| (c, self.chapters[c].paragraphs.len() - 1))
    }

    /// The paragraph's number counted from the start of the text.
    pub fn index_of(&self, (ci, pi): Pos) -> usize {
        self.chapters[..ci]
            .iter()
            .map(|c| c.paragraphs.len())
            .sum::<usize>()
            + pi
    }

    // -- chapters -------------------------------------------------------------

    /// (chapter index, title) of every chapter that has text.
    pub fn chapter_list(&self) -> Vec<(usize, Option<&str>)> {
        self.chapters
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.paragraphs.is_empty())
            .map(|(i, c)| (i, c.title.as_deref()))
            .collect()
    }

    pub fn next_chapter(&self, pos: Pos) -> Option<Pos> {
        (pos.0 + 1..self.chapters.len())
            .find(|&c| !self.chapters[c].paragraphs.is_empty())
            .map(|c| (c, 0))
    }

    /// The start of this chapter, or of the previous one when already there.
    pub fn prev_chapter(&self, pos: Pos) -> Option<Pos> {
        if pos.1 > 0 {
            return Some((pos.0, 0));
        }
        (0..pos.0)
            .rev()
            .find(|&c| !self.chapters[c].paragraphs.is_empty())
            .map(|c| (c, 0))
    }

    // -- search ---------------------------------------------------------------

    /// First paragraph from `start` (inclusive) on containing `query`.
    ///
    /// Searches toward the end (or the beginning when `backward`) and wraps
    /// around once. Returns the paragraph and whether the search wrapped.
    pub fn find(&self, query: &str, start: Pos, backward: bool) -> Option<(Pos, bool)> {
        let needle = fold(query.trim());
        if needle.is_empty() {
            return None;
        }
        let order: Vec<Pos> = self
            .chapters
            .iter()
            .enumerate()
            .flat_map(|(ci, c)| (0..c.paragraphs.len()).map(move |pi| (ci, pi)))
            .collect();
        if order.is_empty() {
            return None;
        }
        let n = order.len() as isize;
        let here = order.iter().position(|&p| p == start).unwrap_or(0) as isize;
        for k in 0..n {
            let i = if backward { here - k } else { here + k };
            let wrapped = i < 0 || i >= n;
            let pos = order[i.rem_euclid(n) as usize];
            if fold(&self.chapters[pos.0].paragraphs[pos.1].text).contains(&needle) {
                return Some((pos, wrapped));
            }
        }
        None
    }
}

/// Case-folded text with one char per input char; Turkish dotted/dotless i count as plain i.
pub fn fold(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if matches!(ch, 'İ' | 'I' | 'ı') {
                return 'i';
            }
            let mut low = ch.to_lowercase();
            match (low.next(), low.next()) {
                (Some(c), None) => c,
                _ => ch, // lowercases to several chars: keep it, so offsets still line up
            }
        })
        .collect()
}

/// Non-overlapping (start, end) char ranges where `query` occurs in `text`, ignoring case.
pub fn match_spans(text: &str, query: &str) -> Vec<(usize, usize)> {
    let needle: Vec<char> = fold(query.trim()).chars().collect();
    if needle.is_empty() {
        return vec![];
    }
    let hay: Vec<char> = fold(text).chars().collect();
    let mut spans = vec![];
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == needle[..] {
            spans.push((i, i + needle.len()));
            i += needle.len();
        } else {
            i += 1;
        }
    }
    spans
}

/// The first 16 hex digits of the content's SHA-256: the key a book's place is saved under.
pub fn book_id(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(chapters: &[(Option<&str>, &[&str])]) -> Book {
        Book {
            id: "id".into(),
            chapters: chapters
                .iter()
                .map(|(t, ps)| Chapter {
                    title: t.map(String::from),
                    paragraphs: ps.iter().map(|p| Paragraph::new(*p)).collect(),
                })
                .collect(),
        }
    }

    // -- test_book.py --

    #[test]
    fn navigation_crosses_chapters() {
        let b = book(&[(Some("1"), &["a", "b"]), (None, &["c"])]);
        assert_eq!(b.next_pos((0, 1)), Some((1, 0)));
        assert_eq!(b.prev_pos((1, 0)), Some((0, 1)));
        assert_eq!(b.next_pos((1, 0)), None);
        assert_eq!(b.prev_pos((0, 0)), None);
        assert_eq!(b.paragraph_count(), 3);
        assert_eq!(b.index_of((1, 0)), 2);
        assert_eq!(b.paragraph((1, 0)).unwrap().text, "c");
    }

    #[test]
    fn book_id_is_16_hex_of_sha256() {
        assert_eq!(book_id(b"abc"), "ba7816bf8f01cfea");
    }

    // -- test_navigation.py --

    fn sample() -> Book {
        book(&[
            (Some("Bir"), &["alfa beta", "gama"]),
            (Some("Boş"), &[]),
            (Some("İki"), &["delta İstanbul", "epsilon"]),
            (None, &["zeta alfa"]),
        ])
    }

    #[test]
    fn chapter_list_skips_empty_chapters() {
        assert_eq!(
            sample().chapter_list(),
            vec![(0, Some("Bir")), (2, Some("İki")), (3, None)]
        );
    }

    #[test]
    fn next_chapter_goes_to_the_first_paragraph_of_the_next_non_empty_one() {
        let b = sample();
        assert_eq!(b.next_chapter((0, 1)), Some((2, 0)));
        assert_eq!(b.next_chapter((2, 0)), Some((3, 0)));
        assert_eq!(b.next_chapter((3, 0)), None);
    }

    #[test]
    fn prev_chapter_goes_to_this_chapters_start_first_then_the_previous_one() {
        let b = sample();
        assert_eq!(b.prev_chapter((2, 1)), Some((2, 0)));
        assert_eq!(b.prev_chapter((2, 0)), Some((0, 0)));
        assert_eq!(b.prev_chapter((0, 0)), None);
    }

    #[test]
    fn fold_keeps_length_and_ignores_case_and_turkish_dotted_i() {
        for s in ["İstanbul", "ISTANBUL", "ıstanbul", "Straße"] {
            assert_eq!(fold(s).chars().count(), s.chars().count());
        }
        assert_eq!(fold("İstanbul"), fold("istanbul"));
        assert_eq!(fold("istanbul"), fold("ISTANBUL"));
        assert_eq!(fold("ISTANBUL"), fold("ıstanbul"));
    }

    #[test]
    fn find_forward_starts_at_the_given_paragraph_and_wraps() {
        let b = sample();
        assert_eq!(b.find("alfa", (0, 0), false), Some(((0, 0), false)));
        assert_eq!(b.find("alfa", (0, 1), false), Some(((3, 0), false)));
        assert_eq!(b.find("beta", (2, 0), false), Some(((0, 0), true)));
    }

    #[test]
    fn find_backward() {
        let b = sample();
        assert_eq!(b.find("alfa", (2, 1), true), Some(((0, 0), false)));
        assert_eq!(b.find("zeta", (0, 0), true), Some(((3, 0), true)));
    }

    #[test]
    fn find_is_case_and_dotted_i_insensitive() {
        assert_eq!(
            sample().find("istanbul", (0, 0), false),
            Some(((2, 0), false))
        );
    }

    #[test]
    fn find_missing_or_empty_returns_none() {
        let b = sample();
        assert_eq!(b.find("omega", (0, 0), false), None);
        assert_eq!(b.find("  ", (0, 0), false), None);
    }

    #[test]
    fn find_ranges_in_a_string() {
        assert_eq!(
            match_spans("Alfa beta ALFA", "alfa"),
            vec![(0, 4), (10, 14)]
        );
        assert_eq!(match_spans("abc", ""), vec![]);
    }
}
