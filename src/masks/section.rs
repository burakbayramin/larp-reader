//! Generic names for the lines that mark a new chapter (`Mask::transition`).
//!
//! Chosen by the chapter's index, never from its title or text: stable from one
//! drawing to the next, different from one chapter to the next.

/// Module-ish names, shared by every mask's transition.
const NAMES: &[&str] = &[
    "chunks",
    "scheduler",
    "storage",
    "metrics",
    "router",
    "tokens",
    "queue",
    "config",
    "export",
    "auth",
    "search",
    "billing",
    "upload",
    "notify",
    "parser",
    "session",
];

/// A name for chapter `chapter`; `salt` lets one transition use several different ones.
///
/// The step is coprime with the list length, so 16 chapters in a row never repeat.
pub fn name(chapter: usize, salt: usize) -> &'static str {
    NAMES[(chapter * 5 + salt * 3) % NAMES.len()]
}

/// One of `items`, by chapter.
pub fn choose<T: Copy>(items: &[T], chapter: usize) -> T {
    items[chapter % items.len()]
}

/// `digits` hex digits that look like a hash, from the chapter and a salt.
pub fn hex(chapter: usize, salt: &str, digits: usize) -> String {
    let mut out = String::new();
    let mut round = 0u32;
    while out.len() < digits {
        let h = crc32fast::hash(format!("{salt}:{chapter}:{round}").as_bytes());
        out.push_str(&format!("{h:08x}"));
        round += 1;
    }
    out.truncate(digits);
    out
}

/// A small version number, by chapter: `0.3.1`.
pub fn version(chapter: usize) -> String {
    format!("0.{}.{}", 1 + chapter * 3 % 9, chapter * 7 % 10)
}

/// Upper-cased first letter, for languages that name files after classes.
pub fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_differ_between_neighbouring_chapters_and_repeat() {
        for c in 0..40 {
            assert_ne!(name(c, 0), name(c + 1, 0));
            assert_eq!(name(c, 0), name(c, 0));
            assert_ne!(name(c, 0), name(c, 1));
        }
    }

    #[test]
    fn hex_has_the_asked_length_and_is_stable() {
        assert_eq!(hex(3, "sha", 40).len(), 40);
        assert_eq!(hex(3, "sha", 7), hex(3, "sha", 7));
        assert_ne!(hex(3, "sha", 7), hex(4, "sha", 7));
        assert!(hex(9, "x", 12).chars().all(|c| c.is_ascii_hexdigit()));
    }
}
