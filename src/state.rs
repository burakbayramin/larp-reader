//! Config and reading-progress persistence (`state.py`).
//!
//! The files are the Python version's, byte for byte in layout, so moving
//! between the two keeps every saved place. Anything read from disk is
//! untrusted: a bad field falls back to its default, a bad progress entry is
//! skipped, and a file that is not a JSON object is moved aside to `*.bak`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::masks::{LANGS, MASK_NAMES};
use crate::noise::MAX_NOISE;
use crate::pace::{DEFAULT_WPM, clamp_wpm};

pub const HIDE_STYLES: &[&str] = &["stream", "shell"];
/// Colour themes, in the order `t` cycles through them.
pub const THEMES: &[&str] = &[
    "dark",
    "light",
    "dark-daltonized",
    "light-daltonized",
    "dark-ansi",
    "light-ansi",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Config {
    pub mask: String,
    pub lang: String,
    pub noise: u8,
    /// Reading speed for auto scroll, learned from manual paging.
    pub wpm: u32,
    pub auto: bool,
    /// What the hide key shows: streaming output or a shell prompt.
    pub hide: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mask: "code".into(),
            lang: "python".into(),
            noise: 2,
            wpm: DEFAULT_WPM,
            auto: false,
            hide: "stream".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub path: String,
    pub chapter: usize,
    pub paragraph: usize,
    pub bookmarks: Vec<[usize; 2]>,
    pub updated_at: String,
}

pub type Progress = BTreeMap<String, Entry>;

/// `%APPDATA%\fcc` when APPDATA is set (Windows), else `~/.config/fcc`.
pub fn state_dir_for(appdata: Option<OsString>, home: Option<PathBuf>) -> PathBuf {
    match appdata.filter(|a| !a.is_empty()) {
        Some(a) => PathBuf::from(a).join("fcc"),
        None => home.unwrap_or_default().join(".config").join("fcc"),
    }
}

/// The state folder for this user, created if missing.
pub fn state_dir() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let dir = state_dir_for(std::env::var_os("APPDATA"), home);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Write pretty JSON (non-ASCII kept as is) through a temporary file and a rename.
fn atomic_write(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(&format!(
            "{}.",
            path.file_name().unwrap_or_default().to_string_lossy()
        ))
        .suffix(".tmp")
        .tempfile_in(dir)?;
    serde_json::to_writer_pretty(&mut tmp, value)?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?; // the temporary file is removed if this fails
    Ok(())
}

/// The file's JSON object; None if missing, or if unreadable as one (then it is moved to `.bak`).
fn read_object(path: &Path) -> Option<Map<String, Value>> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return None,
    };
    let parsed = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(text).ok());
    match parsed {
        Some(Value::Object(map)) => Some(map),
        _ => {
            let mut bak = path.as_os_str().to_owned();
            bak.push(".bak");
            let _ = fs::rename(path, PathBuf::from(bak));
            None
        }
    }
}

/// A finite number from a JSON number or a numeric string; None otherwise (booleans too).
fn number(value: Option<&Value>) -> Option<f64> {
    let n = match value? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    n.is_finite().then_some(n)
}

fn one_of(value: Option<&Value>, allowed: &[&str]) -> Option<String> {
    value?
        .as_str()
        .filter(|s| allowed.contains(s))
        .map(String::from)
}

/// Config from untrusted JSON: each bad field falls back to (or is clamped into) its valid range.
fn valid_config(obj: &Map<String, Value>) -> Config {
    let mut cfg = Config::default();
    if let Some(mask) = one_of(obj.get("mask"), MASK_NAMES) {
        cfg.mask = mask;
    }
    if let Some(lang) = one_of(obj.get("lang"), LANGS) {
        cfg.lang = lang;
    }
    if let Some(noise) = number(obj.get("noise")) {
        cfg.noise = noise.trunc().clamp(0.0, MAX_NOISE as f64) as u8;
    }
    if let Some(wpm) = number(obj.get("wpm")) {
        cfg.wpm = clamp_wpm(wpm);
    }
    if let Some(Value::Bool(auto)) = obj.get("auto") {
        cfg.auto = *auto;
    }
    if let Some(hide) = one_of(obj.get("hide"), HIDE_STYLES) {
        cfg.hide = hide;
    }
    cfg
}

pub fn load_config(dir: &Path) -> Config {
    read_object(&dir.join("config.json"))
        .map(|o| valid_config(&o))
        .unwrap_or_default()
}

pub fn save_config(dir: &Path, cfg: &Config) -> io::Result<()> {
    atomic_write(&dir.join("config.json"), cfg)
}

/// A non-negative JSON integer (not a float, not a boolean).
fn index(value: Option<&Value>) -> Option<usize> {
    value?.as_u64().map(|n| n as usize)
}

/// One progress entry from untrusted JSON; None if a required field is missing or bad.
///
/// Unknown keys are ignored, missing optional keys get defaults and malformed
/// bookmarks are dropped, so one odd entry never costs the other books' progress.
fn entry(value: &Value) -> Option<Entry> {
    let obj = value.as_object()?;
    let bookmarks = obj
        .get("bookmarks")
        .and_then(Value::as_array)
        .map(|marks| {
            marks
                .iter()
                .filter_map(|m| match m.as_array()?.as_slice() {
                    [c, p] => Some([index(Some(c))?, index(Some(p))?]),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Entry {
        path: obj.get("path")?.as_str()?.to_string(),
        chapter: index(obj.get("chapter"))?,
        paragraph: index(obj.get("paragraph"))?,
        bookmarks,
        updated_at: obj
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub fn load_progress(dir: &Path) -> Progress {
    read_object(&dir.join("progress.json"))
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| Some((k.clone(), entry(v)?)))
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_progress(dir: &Path, data: &Progress) -> io::Result<()> {
    atomic_write(&dir.join("progress.json"), data)
}

/// The most recently read book.
pub fn last_entry(data: &Progress) -> Option<(&String, &Entry)> {
    data.iter()
        .max_by(|a, b| a.1.updated_at.cmp(&b.1.updated_at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into())
            .collect();
        v.sort();
        v
    }

    fn config_with(dir: &Path, fields: Value) -> Config {
        fs::write(dir.join("config.json"), fields.to_string()).unwrap();
        load_config(dir)
    }

    fn cfg(mask: &str, lang: &str, noise: u8, wpm: u32, auto: bool) -> Config {
        Config {
            mask: mask.into(),
            lang: lang.into(),
            noise,
            wpm,
            auto,
            ..Config::default()
        }
    }

    // -- test_state.py --

    #[test]
    fn roundtrip_config() {
        let d = tmp();
        let c = cfg("log", "go", 0, 300, true);
        save_config(d.path(), &c).unwrap();
        assert_eq!(load_config(d.path()), c);
    }

    #[test]
    fn missing_config_returns_defaults_without_bak() {
        let d = tmp();
        assert_eq!(load_config(d.path()), Config::default());
        assert_eq!(Config::default(), cfg("code", "python", 2, 220, false));
        assert!(names(d.path()).is_empty());
    }

    #[test]
    fn config_ignores_unknown_and_fills_missing_keys() {
        let d = tmp();
        let c = config_with(d.path(), json!({"mask": "log", "future_key": 1}));
        assert_eq!(
            c,
            Config {
                mask: "log".into(),
                ..Config::default()
            }
        );
    }

    #[test]
    fn config_file_matches_the_python_layout() {
        let d = tmp();
        save_config(d.path(), &Config::default()).unwrap();
        let text = fs::read_to_string(d.path().join("config.json")).unwrap();
        let keys: Vec<&str> = ["mask", "lang", "noise", "wpm", "auto", "hide"].to_vec();
        let at: Vec<usize> = keys
            .iter()
            .map(|k| text.find(&format!("\"{k}\"")).unwrap())
            .collect();
        assert!(
            at.windows(2).all(|w| w[0] < w[1]),
            "keys keep the Python order"
        );
        assert!(text.contains("\n  \"mask\": \"code\""), "two-space indent");
    }

    #[test]
    fn roundtrip_progress_with_turkish_path() {
        let d = tmp();
        let p = d
            .path()
            .join("Kitaplar")
            .join("Suç ve Ceza.epub")
            .to_string_lossy()
            .to_string();
        let mut data = Progress::new();
        data.insert(
            p.clone(),
            Entry {
                path: p,
                chapter: 3,
                paragraph: 7,
                bookmarks: vec![[1, 2], [3, 4]],
                updated_at: "2026-11-11T10:00:00".into(),
            },
        );
        save_progress(d.path(), &data).unwrap();
        assert_eq!(load_progress(d.path()), data);
        let raw = fs::read_to_string(d.path().join("progress.json")).unwrap();
        assert!(raw.contains("Suç ve Ceza")); // not \u-escaped
    }

    #[test]
    fn reads_a_file_the_python_version_wrote() {
        let d = tmp();
        let py = "{\n  \"0123456789abcdef\": {\n    \"path\": \"C:/k/Suç.epub\",\n    \"chapter\": 2,\n    \
                  \"paragraph\": 5,\n    \"bookmarks\": [\n      [\n        0,\n        1\n      ]\n    ],\n    \
                  \"updated_at\": \"2026-11-11T10:00:00\"\n  }\n}";
        fs::write(d.path().join("progress.json"), py).unwrap();
        let e = &load_progress(d.path())["0123456789abcdef"];
        assert_eq!(
            (e.chapter, e.paragraph, e.bookmarks.clone()),
            (2, 5, vec![[0, 1]])
        );
    }

    #[test]
    fn corrupt_json_backed_up_and_defaults_returned() {
        let d = tmp();
        fs::write(d.path().join("config.json"), "{not json").unwrap();
        fs::write(d.path().join("progress.json"), "{not json").unwrap();
        assert_eq!(load_config(d.path()), Config::default());
        assert!(load_progress(d.path()).is_empty());
        assert_eq!(
            fs::read_to_string(d.path().join("config.json.bak")).unwrap(),
            "{not json"
        );
        assert!(d.path().join("progress.json.bak").exists());
        assert!(!d.path().join("config.json").exists());
    }

    #[test]
    fn write_leaves_only_the_target_file() {
        let d = tmp();
        save_config(d.path(), &Config::default()).unwrap();
        save_config(d.path(), &cfg("log", "go", 1, 200, false)).unwrap();
        assert_eq!(names(d.path()), vec!["config.json"]);
    }

    #[test]
    fn last_entry_picks_newest() {
        let e = |p: &str, at: &str| Entry {
            path: p.into(),
            chapter: 0,
            paragraph: 0,
            bookmarks: vec![],
            updated_at: at.into(),
        };
        let mut data = Progress::new();
        data.insert("a".into(), e("a", "2026-01-01T00:00:00"));
        data.insert("b".into(), e("b", "2026-06-01T00:00:00"));
        data.insert("c".into(), e("c", "2026-03-01T00:00:00"));
        assert_eq!(last_entry(&data).unwrap().0, "b");
        assert!(last_entry(&Progress::new()).is_none());
    }

    #[test]
    fn state_dir_uses_appdata_else_home_config() {
        let home = PathBuf::from("/home/x");
        assert_eq!(
            state_dir_for(Some("C:/Users/x/AppData".into()), Some(home.clone())),
            PathBuf::from("C:/Users/x/AppData/fcc")
        );
        assert_eq!(
            state_dir_for(None, Some(home.clone())),
            home.join(".config").join("fcc")
        );
        assert_eq!(
            state_dir_for(Some("".into()), Some(home.clone())),
            home.join(".config").join("fcc")
        );
    }

    #[test]
    fn invalid_utf8_backed_up_and_defaults_returned() {
        let d = tmp();
        fs::write(d.path().join("config.json"), b"\xff\xfe\x00").unwrap();
        fs::write(d.path().join("progress.json"), b"\xff\xfe\x00").unwrap();
        assert_eq!(load_config(d.path()), Config::default());
        assert!(load_progress(d.path()).is_empty());
        assert_eq!(
            fs::read(d.path().join("config.json.bak")).unwrap(),
            b"\xff\xfe\x00"
        );
        assert_eq!(
            fs::read(d.path().join("progress.json.bak")).unwrap(),
            b"\xff\xfe\x00"
        );
        assert!(!d.path().join("config.json").exists());
        assert!(!d.path().join("progress.json").exists());
    }

    #[test]
    fn config_noise_coerced_and_clamped() {
        let d = tmp();
        let default = Config::default().noise;
        for (raw, expected) in [
            (json!(9), 5),
            (json!(-3), 0),
            (json!("2"), 2),
            (json!("4.0"), 4),
            (json!(3.7), 3),
            (json!("lots"), default),
            (json!(null), default),
            (json!(true), default),
            (json!([1]), default),
        ] {
            assert_eq!(
                config_with(d.path(), json!({ "noise": raw })).noise,
                expected,
                "{raw}"
            );
        }
    }

    #[test]
    fn config_wpm_coerced_and_clamped() {
        let d = tmp();
        let default = Config::default().wpm;
        for (raw, expected) in [
            (json!(300), 300),
            (json!(5000), 1500),
            (json!(10), 60),
            (json!("250"), 250),
            (json!(249.6), 250),
            (json!(-1), 60),
            (json!("fast"), default),
            (json!(null), default),
            (json!("nan"), default),
            (json!(true), default),
        ] {
            assert_eq!(
                config_with(d.path(), json!({ "wpm": raw })).wpm,
                expected,
                "{raw}"
            );
        }
    }

    #[test]
    fn old_speed_setting_is_ignored() {
        let d = tmp();
        assert_eq!(
            config_with(d.path(), json!({"speed": 3.0})),
            Config::default()
        );
    }

    #[test]
    fn config_unknown_mask_and_lang_fall_back_to_defaults() {
        let d = tmp();
        let c = config_with(
            d.path(),
            json!({"mask": "nope", "lang": "cobol", "noise": 1}),
        );
        assert_eq!(
            (c.mask.as_str(), c.lang.as_str(), c.noise),
            ("code", "python", 1)
        );
        let c = config_with(d.path(), json!({"mask": 3, "lang": null}));
        assert_eq!((c.mask.as_str(), c.lang.as_str()), ("code", "python"));
    }

    #[test]
    fn config_valid_values_kept() {
        let d = tmp();
        let c = config_with(
            d.path(),
            json!({"mask": "log", "lang": "go", "noise": 0, "wpm": 120, "auto": true}),
        );
        assert_eq!(c, cfg("log", "go", 0, 120, true));
    }

    #[test]
    fn config_auto_must_be_bool() {
        let d = tmp();
        assert!(!config_with(d.path(), json!({"auto": "yes"})).auto);
        assert!(!config_with(d.path(), json!({"auto": 1})).auto);
    }

    #[test]
    fn progress_parsed_per_entry_and_not_quarantined() {
        let d = tmp();
        let good = json!({"path": "C:/a.epub", "chapter": 1, "paragraph": 2, "bookmarks": [[0, 1]], "updated_at": "2026-11-01T10:00:00"});
        let mut extra = good.clone();
        extra["future_field"] = json!("x");
        let mut badtype = good.clone();
        badtype["chapter"] = json!("one");
        let mut badmarks = good.clone();
        badmarks["bookmarks"] = json!([[1, 2], "zz", [3], [true, 1], [4, 5]]);
        let mut floatidx = good.clone();
        floatidx["paragraph"] = json!(2.0);
        let raw = json!({
            "good": good,
            "extra": extra,
            "nomarks": {"path": "C:/b.epub", "chapter": 0, "paragraph": 5, "updated_at": "2026-11-02T10:00:00"},
            "nodate": {"path": "C:/c.epub", "chapter": 0, "paragraph": 1},
            "nochapter": {"path": "C:/d.epub", "paragraph": 1, "bookmarks": [], "updated_at": "x"},
            "badtype": badtype,
            "notdict": [1, 2, 3],
            "badmarks": badmarks,
            "floatidx": floatidx,
        });
        fs::write(d.path().join("progress.json"), raw.to_string()).unwrap();
        let data = load_progress(d.path());
        assert!(!d.path().join("progress.json.bak").exists());
        let keys: Vec<&str> = data.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["badmarks", "extra", "good", "nodate", "nomarks"]);
        assert_eq!(data["good"], data["extra"]);
        assert_eq!(data["good"].bookmarks, vec![[0, 1]]);
        assert!(data["nomarks"].bookmarks.is_empty());
        assert_eq!(data["nodate"].updated_at, "");
        assert_eq!(data["badmarks"].bookmarks, vec![[1, 2], [4, 5]]);
        // saving keeps every readable book's progress
        save_progress(d.path(), &data).unwrap();
        assert_eq!(load_progress(d.path()).len(), data.len());
    }

    #[test]
    fn progress_top_level_not_object_is_quarantined() {
        let d = tmp();
        fs::write(d.path().join("progress.json"), "[1, 2]").unwrap();
        assert!(load_progress(d.path()).is_empty());
        assert!(d.path().join("progress.json.bak").exists());
    }
}
