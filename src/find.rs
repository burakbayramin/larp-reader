//! `larpr find`: readable files in the usual folders, newest first, and the plain
//! numbered list the terminal picker shows before the full-screen app starts.
//!
//! The list names files on purpose: it is printed in the ordinary terminal, never
//! inside the app, whose screens must not show a file name.

use std::collections::HashSet;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use unicode_width::UnicodeWidthStr;

use crate::loaders::{has_suffix, supported_extensions};

/// How many folder levels below a root are searched.
pub const MAX_DEPTH: usize = 4;
/// Folders never entered (hidden ones, starting with a dot, are skipped too).
const SKIPPED_DIRS: [&str; 2] = ["node_modules", "target"];
/// Types too common in the usual folders to list without a query or an explicit folder.
const NOISY: [&str; 6] = [".txt", ".md", ".markdown", ".html", ".htm", ".xhtml"];
/// Directory entries looked at in one search at most, so a huge tree cannot hang it.
const MAX_ENTRIES: usize = 200_000;
/// The widest file name the list shows before shortening it in the middle.
const NAME_WIDTH: usize = 48;
/// The widest folder the list shows before keeping only its end.
const FOLDER_WIDTH: usize = 40;

/// A folder to search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub path: PathBuf,
    /// Whether its subfolders are searched too (to `MAX_DEPTH`).
    pub recursive: bool,
    /// Named by the user: every supported type is listed, noisy ones included.
    pub explicit: bool,
}

impl Root {
    /// A folder the user named.
    pub fn explicit(path: impl Into<PathBuf>) -> Root {
        Root {
            path: path.into(),
            recursive: true,
            explicit: true,
        }
    }

    fn usual(path: PathBuf, recursive: bool) -> Root {
        Root {
            path,
            recursive,
            explicit: false,
        }
    }
}

/// A file a search turned up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
}

/// What to search: where, and an optional part of the file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    pub roots: Vec<Root>,
    pub query: Option<String>,
}

impl Search {
    /// The usual folders under the home folder.
    pub fn usual(query: Option<String>) -> Search {
        Search {
            roots: default_roots(),
            query,
        }
    }

    pub fn run(&self, limit: usize) -> Vec<Found> {
        find_books(&self.roots, self.query.as_deref(), limit)
    }
}

/// The home folder: `HOME`, else `USERPROFILE` (Windows).
pub fn home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(std::env::var_os)
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Documents, Downloads, Desktop and Books under `home` (and under OneDrive, which
/// Windows often moves Documents and Desktop to), then `home` itself without its
/// subfolders.
pub fn usual_roots(home: &Path, onedrive: Option<&Path>) -> Vec<Root> {
    let mut roots: Vec<Root> = ["Documents", "Downloads", "Desktop", "Books"]
        .into_iter()
        .map(|n| Root::usual(home.join(n), true))
        .collect();
    if let Some(od) = onedrive {
        for n in ["Documents", "Desktop"] {
            roots.push(Root::usual(od.join(n), true));
        }
    }
    roots.push(Root::usual(home.to_path_buf(), false));
    roots
}

/// `usual_roots` for this user, from the environment; none without a home folder.
pub fn default_roots() -> Vec<Root> {
    let Some(home) = home_dir() else {
        return vec![];
    };
    let onedrive = std::env::var_os("OneDrive")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    usual_roots(&home, onedrive.as_deref())
}

/// Files some loader opens under `roots` whose name contains `query` (any case),
/// newest-modified first, at most `limit`. Without a query, plain text and HTML files
/// are left out of the usual folders (there are too many); a folder the user named
/// lists them all.
pub fn find_books(roots: &[Root], query: Option<&str>, limit: usize) -> Vec<Found> {
    let extensions = supported_extensions();
    let query = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut budget = MAX_ENTRIES;
    for root in roots {
        let all_types = root.explicit || query.is_some();
        let wanted = |path: &Path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            has_suffix(path, &extensions)
                && (all_types || !has_suffix(path, &NOISY))
                && query.as_ref().is_none_or(|q| name.contains(q.as_str()))
        };
        let depth = if root.recursive { MAX_DEPTH } else { 0 };
        let mut visit = |path: PathBuf, meta: std::fs::Metadata| {
            if seen.insert(path.clone()) {
                found.push(Found {
                    path,
                    size: meta.len(),
                    modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                });
            }
        };
        scan(&root.path, depth, &mut budget, &wanted, &mut visit);
    }
    found.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.path.cmp(&b.path))
    });
    found.truncate(limit);
    found
}

/// Visit the wanted files in `dir`, then search its subfolders `depth` levels down.
fn scan(
    dir: &Path,
    depth: usize,
    budget: &mut usize,
    wanted: &dyn Fn(&Path) -> bool,
    visit: &mut dyn FnMut(PathBuf, std::fs::Metadata),
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        if *budget == 0 {
            return;
        }
        *budget -= 1;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            // symlinked folders are not followed: they could loop
            if depth > 0 && !SKIPPED_DIRS.contains(&name.as_ref()) {
                subdirs.push(path);
            }
        } else if wanted(&path) {
            // follows a symlinked file to its target
            if let Ok(meta) = std::fs::metadata(&path)
                && meta.is_file()
            {
                visit(path, meta);
            }
        }
    }
    subdirs.sort();
    for sub in subdirs {
        scan(&sub, depth - 1, budget, wanted, visit);
    }
}

/// `1234567` → `1.2 MB`.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64 / 1024.0;
    let mut unit = 0;
    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }
    format!("{size:.1} {}", UNITS[unit])
}

/// `dir` with the home folder written `~`, and only its end kept when it is wider
/// than `max` columns.
pub fn short_folder(dir: &Path, home: Option<&Path>, max: usize) -> String {
    let text = match home.and_then(|h| dir.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => dir.display().to_string(),
    };
    keep_end(&text, max)
}

/// `text`, or `…` and as much of its end as fits in `max` columns.
fn keep_end(text: &str, max: usize) -> String {
    if text.width() <= max {
        return text.to_string();
    }
    let mut tail: Vec<char> = Vec::new();
    let mut width = 1; // the ellipsis
    for c in text.chars().rev() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if width + w > max {
            break;
        }
        width += w;
        tail.push(c);
    }
    std::iter::once('…').chain(tail.into_iter().rev()).collect()
}

/// `name`, shortened in the middle (so the type stays visible) to `max` columns.
fn keep_ends(name: &str, max: usize) -> String {
    if name.width() <= max {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let tail_len = 12.min(chars.len());
    let tail: String = chars[chars.len() - tail_len..].iter().collect();
    let mut head = String::new();
    let room = max.saturating_sub(tail.width() + 1);
    for &c in &chars[..chars.len() - tail_len] {
        if (head.clone() + &c.to_string()).width() > room {
            break;
        }
        head.push(c);
    }
    format!("{head}…{tail}")
}

/// The numbered list: number, file name, size, folder; columns lined up.
pub fn listing(found: &[Found], home: Option<&Path>) -> Vec<String> {
    let names: Vec<String> = found
        .iter()
        .map(|f| {
            let name = f
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            keep_ends(&name, NAME_WIDTH)
        })
        .collect();
    let name_width = names.iter().map(|n| n.width()).max().unwrap_or(0);
    let number_width = found.len().to_string().len();
    found
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(i, (f, name))| {
            let folder = f
                .path
                .parent()
                .map(|p| short_folder(p, home, FOLDER_WIDTH))
                .unwrap_or_default();
            let pad = " ".repeat(name_width - name.width());
            format!(
                "{:>number_width$}  {name}{pad}  {:>9}  {folder}",
                i + 1,
                human_size(f.size)
            )
        })
        .collect()
}

/// The roots, one short line: `~/Documents, ~/Downloads, ~ (top level only)`.
pub fn describe_roots(roots: &[Root], home: Option<&Path>) -> String {
    if roots.is_empty() {
        return "nothing (no home folder found: HOME / USERPROFILE is not set)".into();
    }
    roots
        .iter()
        .map(|r| {
            let dir = short_folder(&r.path, home, usize::MAX);
            if r.recursive {
                dir
            } else {
                format!("{dir} (top level only)")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Ask for a number from 1 to `count` until one comes, or nothing (Enter or end of input).
pub fn choose(
    input: &mut impl BufRead,
    out: &mut impl Write,
    count: usize,
) -> io::Result<Option<usize>> {
    loop {
        write!(out, "open which? (number, Enter to cancel): ")?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            writeln!(out)?;
            return Ok(None);
        }
        let line = line.trim();
        if line.is_empty() {
            return Ok(None);
        }
        match line.parse::<usize>() {
            Ok(n) if (1..=count).contains(&n) => return Ok(Some(n - 1)),
            _ => writeln!(
                out,
                "type a number from 1 to {count}, or press Enter to cancel"
            )?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    /// Write `rel` under `root` with `len` bytes, modified `age` seconds before a fixed time.
    fn file(root: &Path, rel: &str, len: usize, age: u64) -> PathBuf {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, vec![b'x'; len]).unwrap();
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000 - age);
        fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(t)
            .unwrap();
        p
    }

    fn names(found: &[Found]) -> Vec<String> {
        found
            .iter()
            .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    fn usual(path: &Path) -> Root {
        Root::usual(path.to_path_buf(), true)
    }

    #[test]
    fn newest_first_and_only_supported_types() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "old.epub", 10, 300);
        file(d.path(), "sub/new.pdf", 10, 100);
        file(d.path(), "mid.mobi", 10, 200);
        file(d.path(), "photo.jpg", 10, 0);
        file(d.path(), "noext", 10, 0);
        let found = find_books(&[usual(d.path())], None, 10);
        assert_eq!(names(&found), ["new.pdf", "mid.mobi", "old.epub"]);
        assert_eq!(found[0].size, 10);
    }

    #[test]
    fn skips_hidden_and_build_folders() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), ".hidden/a.epub", 1, 0);
        file(d.path(), "node_modules/b.epub", 1, 0);
        file(d.path(), "target/c.epub", 1, 0);
        file(d.path(), ".git/d.epub", 1, 0);
        file(d.path(), "._e.epub", 1, 0);
        file(d.path(), "keep/f.epub", 1, 0);
        let found = find_books(&[usual(d.path())], None, 10);
        assert_eq!(names(&found), ["f.epub"]);
    }

    #[test]
    fn stops_four_levels_down() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "1/2/3/4/deep.epub", 1, 0);
        file(d.path(), "1/2/3/4/5/deeper.epub", 1, 0);
        let found = find_books(&[usual(d.path())], None, 10);
        assert_eq!(names(&found), ["deep.epub"]);
    }

    #[test]
    fn a_shallow_root_lists_only_its_own_files() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "top.epub", 1, 0);
        file(d.path(), "sub/inner.epub", 1, 0);
        let root = Root::usual(d.path().to_path_buf(), false);
        assert_eq!(names(&find_books(&[root], None, 10)), ["top.epub"]);
    }

    #[test]
    fn query_matches_part_of_the_name_in_any_case() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "Dune Messiah.epub", 1, 0);
        file(d.path(), "dune.PDF", 1, 10);
        file(d.path(), "Foundation.epub", 1, 0);
        file(d.path(), "dune/other.epub", 1, 0); // the folder's name does not count
        let found = find_books(&[usual(d.path())], Some("DUNE"), 10);
        assert_eq!(names(&found), ["Dune Messiah.epub", "dune.PDF"]);
        let all = find_books(&[usual(d.path())], Some("  "), 10);
        assert_eq!(all.len(), 4, "a blank query matches everything");
    }

    #[test]
    fn noisy_types_need_a_query_or_an_explicit_folder() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "notes.txt", 1, 0);
        file(d.path(), "readme.md", 1, 0);
        file(d.path(), "page.html", 1, 0);
        file(d.path(), "page.htm", 1, 0);
        file(d.path(), "novel.epub", 1, 0);
        let usual_list = find_books(&[usual(d.path())], None, 10);
        assert_eq!(names(&usual_list), ["novel.epub"]);
        let queried = find_books(&[usual(d.path())], Some("notes"), 10);
        assert_eq!(names(&queried), ["notes.txt"]);
        let explicit = find_books(&[Root::explicit(d.path())], None, 10);
        assert_eq!(explicit.len(), 5);
    }

    #[test]
    fn limit_keeps_the_newest_and_duplicates_are_dropped() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..5 {
            file(d.path(), &format!("f{i}.epub"), 1, i);
        }
        let root = usual(d.path());
        let found = find_books(&[root.clone(), root], None, 3);
        assert_eq!(names(&found), ["f0.epub", "f1.epub", "f2.epub"]);
    }

    #[test]
    fn missing_roots_are_skipped() {
        let d = tempfile::tempdir().unwrap();
        file(d.path(), "a.epub", 1, 0);
        let roots = [usual(&d.path().join("nope")), usual(d.path())];
        assert_eq!(names(&find_books(&roots, None, 10)), ["a.epub"]);
    }

    #[test]
    fn usual_roots_are_the_common_folders_then_home_alone() {
        let home = Path::new("h");
        let roots = usual_roots(home, None);
        let paths: Vec<_> = roots.iter().map(|r| r.path.clone()).collect();
        assert_eq!(
            paths,
            [
                home.join("Documents"),
                home.join("Downloads"),
                home.join("Desktop"),
                home.join("Books"),
                home.to_path_buf()
            ]
        );
        assert!(roots[..4].iter().all(|r| r.recursive && !r.explicit));
        assert!(!roots[4].recursive && !roots[4].explicit);
        let with_od = usual_roots(home, Some(Path::new("od")));
        assert_eq!(with_od.len(), 7);
        assert_eq!(with_od[4].path, Path::new("od").join("Documents"));
    }

    #[test]
    fn usual_folders_under_a_home() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        file(h, "Downloads/dl.epub", 1, 1);
        file(h, "Desktop/x/desk.pdf", 1, 2);
        file(h, "Documents/doc.fb2", 1, 3);
        file(h, "Books/a/b/c/d/deep.mobi", 1, 4);
        file(h, "loose.azw3", 1, 5);
        file(h, "Music/song.epub", 1, 0); // home is searched without its subfolders
        file(h, "Downloads/list.txt", 1, 0);
        let found = find_books(&usual_roots(h, None), None, 10);
        assert_eq!(
            names(&found),
            ["dl.epub", "desk.pdf", "doc.fb2", "deep.mobi", "loose.azw3"]
        );
    }

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1_234_567), "1.2 MB");
        assert_eq!(human_size(3 << 30), "3.0 GB");
    }

    #[test]
    fn folders_shorten_home_and_keep_their_end() {
        let home = Path::new("home").join("me");
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(short_folder(&home, Some(&home), 40), "~");
        assert_eq!(
            short_folder(&home.join("Downloads"), Some(&home), 40),
            format!("~{sep}Downloads")
        );
        let other = Path::new("srv").join("files");
        assert_eq!(
            short_folder(&other, Some(&home), 40),
            other.display().to_string()
        );
        let long = home.join("a".repeat(30)).join("tail-of-the-path");
        let short = short_folder(&long, Some(&home), 20);
        assert!(short.starts_with('…'), "{short}");
        assert!(short.ends_with("tail-of-the-path"), "{short}");
        assert_eq!(short.width(), 20);
    }

    #[test]
    fn long_names_keep_their_type() {
        let name = format!("{}.epub", "x".repeat(80));
        let short = keep_ends(&name, 48);
        assert!(short.ends_with(".epub") && short.contains('…'), "{short}");
        assert!(short.width() <= 48);
        assert_eq!(keep_ends("short.epub", 48), "short.epub");
    }

    #[test]
    fn listing_lines_up_columns() {
        let home = Path::new("home");
        let t = SystemTime::UNIX_EPOCH;
        let f = |p: PathBuf, size| Found {
            path: p,
            size,
            modified: t,
        };
        let found: Vec<Found> = (0..10)
            .map(|i| {
                f(
                    home.join("Downloads")
                        .join(format!("b{}.epub", "o".repeat(i))),
                    2048,
                )
            })
            .collect();
        let lines = listing(&found, Some(home));
        assert_eq!(lines.len(), 10);
        assert!(lines[0].starts_with(" 1  b.epub "), "{}", lines[0]);
        assert!(
            lines[9].starts_with("10  booooooooo.epub  "),
            "{}",
            lines[9]
        );
        assert!(lines[0].contains("2.0 KB"));
        let sep = std::path::MAIN_SEPARATOR;
        assert!(lines[0].ends_with(&format!("~{sep}Downloads")));
        let widths: Vec<_> = lines.iter().map(|l| l.width()).collect();
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "{lines:#?}");
    }

    #[test]
    fn roots_are_described_plainly() {
        let home = Path::new("h");
        let text = describe_roots(&usual_roots(home, None), Some(home));
        let sep = std::path::MAIN_SEPARATOR;
        assert!(
            text.starts_with(&format!("~{sep}Documents, ~{sep}Downloads")),
            "{text}"
        );
        assert!(text.ends_with("~ (top level only)"), "{text}");
        assert!(describe_roots(&[], None).contains("HOME"));
    }

    #[test]
    fn choose_takes_a_number_in_range() {
        let ask = |typed: &str, count| {
            let mut out = Vec::new();
            let pick = choose(&mut typed.as_bytes(), &mut out, count).unwrap();
            (pick, String::from_utf8(out).unwrap())
        };
        assert_eq!(ask("2\n", 3).0, Some(1));
        assert_eq!(ask("\n", 3).0, None);
        assert_eq!(ask("", 3).0, None, "end of input cancels");
        let (pick, out) = ask("0\nabc\n9\n3\n", 3);
        assert_eq!(pick, Some(2));
        assert_eq!(
            out.matches("open which? (number, Enter to cancel): ")
                .count(),
            4
        );
        assert_eq!(out.matches("type a number from 1 to 3").count(), 3);
    }
}
