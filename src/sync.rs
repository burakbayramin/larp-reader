//! Progress sync between computers through a shared folder (Dropbox, Syncthing, a
//! network drive): with `sync_dir` set in the config, a book's place and bookmarks
//! go to `<sync_dir>/<id>.json` and come back from it.
//!
//! Books are matched by their content-hash id, so the file's location may differ per
//! machine; the synced file never holds a path, title or file name. When both sides
//! have a place, the newer `updated_at` wins as a whole. Every failure here is
//! swallowed: a missing, unwritable or garbled folder just means no sync.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::state::{Entry, entry_from_value};

/// `<sync_dir>/<id>.json`, or None when sync is off or `id` is not a plain hex id.
fn file_for(sync_dir: &str, id: &str) -> Option<PathBuf> {
    let ok = !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit());
    (!sync_dir.trim().is_empty() && ok)
        .then(|| PathBuf::from(sync_dir.trim()).join(format!("{id}.json")))
}

/// The synced place of book `id`, if there is a readable one.
fn read(sync_dir: &str, id: &str) -> Option<Entry> {
    let bytes = fs::read(file_for(sync_dir, id)?).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    entry_from_value(&value)
}

/// The place to open book `id` at: the synced one if it is newer than `local` (keeping
/// the local file path), else `local`.
pub fn merge(sync_dir: &str, id: &str, local: Option<Entry>) -> Option<Entry> {
    let remote = read(sync_dir, id);
    match (local, remote) {
        (Some(l), Some(r)) if r.updated_at > l.updated_at => Some(Entry { path: l.path, ..r }),
        (Some(l), _) => Some(l),
        (None, r) => r,
    }
}

/// Write `entry` for book `id` unless the folder already holds a newer or equal place.
pub fn push(sync_dir: &str, id: &str, entry: &Entry) {
    let Some(path) = file_for(sync_dir, id) else {
        return;
    };
    if read(sync_dir, id).is_some_and(|r| r.updated_at >= entry.updated_at) {
        return;
    }
    let value = json!({
        "path": "",
        "chapter": entry.chapter,
        "paragraph": entry.paragraph,
        "bookmarks": entry.bookmarks,
        "updated_at": entry.updated_at,
    });
    let _ = write_atomic(&path, &value.to_string());
}

fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    fs::create_dir_all(dir)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".sync.")
        .suffix(".tmp")
        .tempfile_in(dir)?;
    tmp.write_all(text.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef";

    fn entry(path: &str, chapter: usize, at: &str) -> Entry {
        Entry {
            path: path.into(),
            chapter,
            paragraph: chapter + 1,
            bookmarks: vec![[chapter, 0]],
            updated_at: at.into(),
        }
    }

    fn dir_str(d: &tempfile::TempDir) -> String {
        d.path().to_string_lossy().into_owned()
    }

    #[test]
    fn off_when_unset() {
        assert!(merge("", ID, None).is_none());
        let e = entry("/a", 1, "2026-01-01T00:00:00");
        push("", ID, &e);
        assert_eq!(merge("  ", ID, Some(e.clone())), Some(e));
    }

    #[test]
    fn push_then_merge_on_another_machine() {
        let d = tempfile::tempdir().unwrap();
        let s = dir_str(&d);
        push(&s, ID, &entry("/home/a/x.epub", 3, "2026-02-01T10:00:00"));
        // a fresh machine: no local place, its own path is unknown
        let got = merge(&s, ID, None).unwrap();
        assert_eq!((got.chapter, got.paragraph), (3, 4));
        assert_eq!(got.bookmarks, vec![[3, 0]]);
        assert_eq!(got.path, "");
        // the synced file never holds the path
        let raw = fs::read_to_string(d.path().join(format!("{ID}.json"))).unwrap();
        assert!(!raw.contains("x.epub"), "{raw}");
    }

    #[test]
    fn newest_timestamp_wins_and_keeps_the_local_path() {
        let d = tempfile::tempdir().unwrap();
        let s = dir_str(&d);
        push(&s, ID, &entry("/theirs", 5, "2026-03-01T00:00:00"));
        let older = entry("D:/mine.epub", 1, "2026-02-01T00:00:00");
        let got = merge(&s, ID, Some(older)).unwrap();
        assert_eq!((got.chapter, got.path.as_str()), (5, "D:/mine.epub"));
        let newer = entry("D:/mine.epub", 9, "2026-04-01T00:00:00");
        assert_eq!(merge(&s, ID, Some(newer.clone())), Some(newer.clone()));
        // and pushing the newer one replaces the older synced place, not the other way round
        push(&s, ID, &newer);
        push(&s, ID, &entry("/theirs", 5, "2026-03-01T00:00:00"));
        assert_eq!(merge(&s, ID, None).unwrap().chapter, 9);
    }

    #[test]
    fn failures_are_silent() {
        let d = tempfile::tempdir().unwrap();
        // a file where the folder should be: nothing can be written, nothing panics
        let blocker = d.path().join("f");
        fs::write(&blocker, "x").unwrap();
        let s = dir_str(&d) + "/f/sub";
        let e = entry("/a", 1, "2026-01-01T00:00:00");
        push(&s, ID, &e);
        assert_eq!(merge(&s, ID, Some(e.clone())), Some(e));
        // garbled content is ignored
        let s = dir_str(&d);
        fs::write(d.path().join(format!("{ID}.json")), "{not json").unwrap();
        assert!(merge(&s, ID, None).is_none());
    }

    #[test]
    fn ids_cannot_escape_the_folder() {
        let d = tempfile::tempdir().unwrap();
        let s = dir_str(&d);
        push(&s, "../evil", &entry("/a", 1, "2026-01-01T00:00:00"));
        assert!(merge(&s, "../evil", None).is_none());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }
}
