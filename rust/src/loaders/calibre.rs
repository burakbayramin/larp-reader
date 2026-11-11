//! Last resort (`loaders/calibre.py`): convert with Calibre's `ebook-convert` to EPUB, then read that.
//!
//! Finding and running the converter are swappable, so the tests can stand
//! in for Calibre the way the Python tests patch `shutil.which` and
//! `subprocess.run`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::epub::EpubLoader;
use super::{Loader, find_program};
use crate::book::{Book, book_id};
use crate::errors::FccError;

pub const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, PartialEq, Eq)]
pub enum RunResult {
    Exited(i32),
    TimedOut,
    Failed(String),
}

type Find = Box<dyn Fn() -> Option<PathBuf>>;
type Run = Box<dyn Fn(&[OsString], Duration) -> RunResult>;

pub struct CalibreLoader {
    find: Find,
    run: Run,
}

/// Run `argv` without a shell, its output discarded, killing it after `timeout`.
fn run_command(argv: &[OsString], timeout: Duration) -> RunResult {
    let mut child = match Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return RunResult::Failed(e.kind().to_string()),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return RunResult::Exited(status.code().unwrap_or(-1)),
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return RunResult::TimedOut;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return RunResult::Failed(e.kind().to_string()),
        }
    }
}

impl CalibreLoader {
    /// The real thing: `ebook-convert` from PATH.
    pub fn system() -> Self {
        CalibreLoader {
            find: Box::new(|| find_program("ebook-convert")),
            run: Box::new(run_command),
        }
    }

    pub fn with(find: Find, run: Run) -> Self {
        CalibreLoader { find, run }
    }

    pub fn available(&self) -> bool {
        (self.find)().is_some()
    }
}

fn unsupported(msg: String) -> FccError {
    FccError::UnsupportedFormat(msg)
}

impl Loader for CalibreLoader {
    fn name(&self) -> &'static str {
        "calibre"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn can_load(&self, _path: &Path, _head: &[u8]) -> bool {
        self.available()
    }

    fn load(&self, path: &Path) -> Result<Book, FccError> {
        let exe = (self.find)().ok_or_else(|| unsupported("ebook-convert bulunamadı".into()))?;
        let tmp = tempfile::tempdir().map_err(|e| FccError::from_io(&e))?;
        let out = tmp.path().join("converted.epub");
        let argv = [
            exe.into_os_string(),
            path.as_os_str().to_owned(),
            out.as_os_str().to_owned(),
        ];
        match (self.run)(&argv, TIMEOUT) {
            RunResult::Exited(0) => {}
            RunResult::Exited(code) => {
                return Err(unsupported(format!(
                    "ebook-convert başarısız (çıkış kodu {code})"
                )));
            }
            RunResult::TimedOut => {
                return Err(unsupported(format!(
                    "ebook-convert {} sn içinde bitmedi",
                    TIMEOUT.as_secs()
                )));
            }
            RunResult::Failed(why) => {
                return Err(unsupported(format!("ebook-convert çalıştırılamadı: {why}")));
            }
        }
        if !out.is_file() {
            return Err(unsupported("ebook-convert çıktı üretmedi".into()));
        }
        let converted = EpubLoader.load(&out)?;
        let data = std::fs::read(path).map_err(|e| FccError::from_io(&e))?;
        Ok(Book {
            id: book_id(&data),
            chapters: converted.chapters,
        })
        // `tmp` is removed here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loaders::epub::tests::make_epub;
    use crate::loaders::{Loader, load_with};
    use std::cell::RefCell;
    use std::rc::Rc;

    const EXE: &str = "C:/calibre/ebook-convert.exe";

    type Calls = Rc<RefCell<Vec<(Vec<OsString>, Duration)>>>;

    /// Calibre "installed"; running it writes an EPUB to the output path and exits with `code`.
    fn fake(code: i32, write: bool) -> (CalibreLoader, Calls) {
        let calls: Calls = Rc::default();
        let seen = calls.clone();
        let loader = CalibreLoader::with(
            Box::new(|| Some(PathBuf::from(EXE))),
            Box::new(move |argv, timeout| {
                seen.borrow_mut().push((argv.to_vec(), timeout));
                if write {
                    make_epub(Path::new(&argv[2]), None);
                }
                RunResult::Exited(code)
            }),
        );
        (loader, calls)
    }

    fn missing() -> CalibreLoader {
        CalibreLoader::with(Box::new(|| None), Box::new(|_, _| panic!("must not run")))
    }

    fn file(name: &str, data: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(name);
        std::fs::write(&p, data).unwrap();
        (d, p)
    }

    fn chain(last: CalibreLoader) -> Vec<Box<dyn Loader>> {
        let mut v = crate::loaders::loaders();
        v.pop(); // the system Calibre
        v.push(Box::new(last));
        v
    }

    // -- test_calibre_and_chain.py --

    #[test]
    fn calibre_is_last_loader() {
        assert_eq!(crate::loaders::loaders().last().unwrap().name(), "calibre");
    }

    #[test]
    fn calibre_used_when_others_fail() {
        let (_d, p) = file("kitap.djvu", b"AT&TFORM fake djvu");
        let (cal, calls) = fake(0, true);
        let book = load_with(&p, &chain(cal), true).unwrap();
        assert!(book.paragraph_count() > 0);
        let (argv, timeout) = &calls.borrow()[0];
        assert_eq!(argv[0], OsString::from(EXE));
        assert_eq!(argv[1], p.as_os_str());
        assert!(argv[2].to_string_lossy().ends_with(".epub"));
        assert_eq!(*timeout, Duration::from_secs(120));
    }

    #[test]
    fn calibre_book_id_from_original_bytes() {
        let (_d, p) = file("kitap.djvu", b"AT&TFORM original bytes");
        let (cal, _) = fake(0, true);
        assert_eq!(
            load_with(&p, &chain(cal), true).unwrap().id,
            book_id(b"AT&TFORM original bytes")
        );
    }

    #[test]
    fn calibre_temp_dir_cleaned_up() {
        let (_d, p) = file("kitap.djvu", b"x");
        let (cal, calls) = fake(0, true);
        load_with(&p, &chain(cal), true).unwrap();
        let out = PathBuf::from(&calls.borrow()[0].0[2]);
        assert!(!out.parent().unwrap().exists());
    }

    #[test]
    fn calibre_absent_gives_unsupported_with_install_hint() {
        let (_d, p) = file("kitap.djvu", b"AT&TFORM fake djvu");
        let msg = load_with(&p, &chain(missing()), false)
            .unwrap_err()
            .to_string();
        assert!(
            msg.contains(".djvu") && msg.contains("ebook-convert"),
            "{msg}"
        );
    }

    #[test]
    fn calibre_failures_are_unsupported() {
        let (_d, p) = file("a.djvu", b"x");
        let e = fake(1, false).0.load(&p).unwrap_err();
        assert!(
            matches!(&e, FccError::UnsupportedFormat(m) if m.contains('1')),
            "{e}"
        );
        assert!(matches!(
            fake(0, false).0.load(&p),
            Err(FccError::UnsupportedFormat(_))
        )); // no output
        let slow = CalibreLoader::with(
            Box::new(|| Some(EXE.into())),
            Box::new(|_, _| RunResult::TimedOut),
        );
        assert!(matches!(slow.load(&p), Err(FccError::UnsupportedFormat(_))));
    }

    #[test]
    fn calibre_can_load_follows_which() {
        assert!(fake(0, true).0.can_load(Path::new("a.xyz"), b""));
        assert!(!missing().can_load(Path::new("a.xyz"), b""));
    }

    #[test]
    fn drm_not_retried_with_calibre() {
        struct Drm;
        impl Loader for Drm {
            fn name(&self) -> &'static str {
                "drm"
            }
            fn extensions(&self) -> &'static [&'static str] {
                &[".djvu"]
            }
            fn can_load(&self, _: &Path, _: &[u8]) -> bool {
                true
            }
            fn load(&self, _: &Path) -> Result<Book, FccError> {
                Err(FccError::DrmProtected("DRM korumalı, açılamaz".into()))
            }
        }
        let (_d, p) = file("a.djvu", b"x");
        let never = CalibreLoader::with(
            Box::new(|| Some(EXE.into())),
            Box::new(|_, _| panic!("calibre after DRM")),
        );
        let chain: Vec<Box<dyn Loader>> = vec![Box::new(Drm), Box::new(never)];
        assert!(matches!(
            load_with(&p, &chain, true),
            Err(FccError::DrmProtected(_))
        ));
    }

    #[test]
    fn no_install_hint_when_calibre_present_but_failed() {
        let (_d, p) = file("a.djvu", b"x");
        let msg = load_with(&p, &chain(fake(2, false).0), true)
            .unwrap_err()
            .to_string();
        assert!(!msg.contains("kurarak") && msg.contains(".djvu"), "{msg}");
    }

    #[test]
    fn corrupt_but_sniffable_is_unsupported() {
        for (name, content) in [
            (
                "bad.docx",
                [b"PK\x03\x04".as_slice(), &b"junk".repeat(100)].concat(),
            ),
            (
                "bad.odt",
                [b"PK\x03\x04".as_slice(), &b"junk".repeat(100)].concat(),
            ),
            (
                "bad.epub",
                [b"PK\x03\x04".as_slice(), &b"junk".repeat(100)].concat(),
            ),
            ("bad.rtf", [b"{\rtf1".as_slice(), &[0x8f; 256]].concat()),
        ] {
            let (_d, p) = file(name, &content);
            assert!(
                matches!(
                    load_with(&p, &chain(missing()), false),
                    Err(FccError::UnsupportedFormat(_))
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn real_runner_reports_a_missing_program_and_exit_codes() {
        let missing = run_command(&["fcc-no-such-program-xyz".into()], Duration::from_secs(5));
        assert!(matches!(missing, RunResult::Failed(_)));
        if cfg!(unix) {
            assert_eq!(
                run_command(
                    &["sh".into(), "-c".into(), "exit 3".into()],
                    Duration::from_secs(5)
                ),
                RunResult::Exited(3)
            );
            assert_eq!(
                run_command(&["sleep".into(), "5".into()], Duration::from_millis(300)),
                RunResult::TimedOut
            );
        }
    }
}
