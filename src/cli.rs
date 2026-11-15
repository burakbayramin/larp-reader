//! Command line (`cli.py`): `larpr build [FILE] [--mask …] [--lang …] [--noise …] [--auto] [--wpm …] [--hide …]`.
//!
//! Like `--version`, `--help` reads like a build tool's and never says what the
//! program really does.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::builder::PossibleValuesParser;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::errors::FccError;
use crate::masks::{LANGS, MASK_NAMES};
use crate::noise::MAX_NOISE;
use crate::pace::{MAX_WPM, MIN_WPM};
use crate::state::{
    Config, HIDE_STYLES, THEMES, last_entry, load_config, load_progress, save_config,
};

#[derive(Parser, Debug)]
#[command(
    name = "larpr",
    about = "larpr build tool",
    disable_version_flag = true
)]
struct Cli {
    /// Print version
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    version: Option<bool>,
    #[command(subcommand)]
    cmd: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// build a target
    Build(BuildArgs),
}

#[derive(Args, Debug, Default)]
struct BuildArgs {
    /// input file (default: last used)
    file: Option<PathBuf>,
    /// output style
    #[arg(long, value_parser = PossibleValuesParser::new(MASK_NAMES))]
    mask: Option<String>,
    /// language for the code style
    #[arg(long, value_parser = PossibleValuesParser::new(LANGS))]
    lang: Option<String>,
    /// amount of filler output
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=MAX_NOISE as i64))]
    noise: Option<u8>,
    /// advance automatically
    #[arg(long)]
    auto: bool,
    /// words per minute for auto advance
    #[arg(long, value_parser = clap::value_parser!(u32).range(MIN_WPM as i64..=MAX_WPM as i64))]
    wpm: Option<u32>,
    /// what the hide key shows
    #[arg(long, value_parser = PossibleValuesParser::new(HIDE_STYLES))]
    hide: Option<String>,
    /// colour theme
    #[arg(long, value_parser = PossibleValuesParser::new(THEMES))]
    theme: Option<String>,
}

fn command() -> clap::Command {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let version = VERSION.get_or_init(|| crate::version().trim_start_matches("larpr ").to_string());
    Cli::command().version(version.as_str())
}

/// What `main` should do after parsing.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Open this file with this config.
    Run(PathBuf, Config),
    /// Nothing to open: print the error and exit with status 1.
    Fail(FccError),
}

/// Parse `argv` (without the program name), apply flags to the saved config, save it and
/// pick the file to open. Bad flags come back as a clap error before anything is saved.
pub fn resolve<I, T>(argv: I, state_dir: &Path) -> Result<Outcome, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let argv = std::iter::once(OsString::from("larpr")).chain(argv.into_iter().map(Into::into));
    let matches = command().try_get_matches_from(argv)?;
    let cli = Cli::from_arg_matches(&matches)?;
    let args = match cli.cmd {
        Some(Command::Build(a)) => a,
        None => BuildArgs::default(),
    };

    let mut config = load_config(state_dir);
    if let Some(m) = args.mask {
        config.mask = m;
    }
    if let Some(l) = args.lang {
        config.lang = l;
    }
    if let Some(n) = args.noise {
        config.noise = n;
    }
    if args.auto {
        config.auto = true;
    }
    if let Some(w) = args.wpm {
        config.wpm = w;
    }
    if let Some(h) = args.hide {
        config.hide = h;
    }
    if let Some(t) = args.theme {
        config.theme = t;
    }
    let _ = save_config(state_dir, &config); // best effort: the flags still apply to this run

    let path = match args.file {
        Some(p) => p,
        None => match last_entry(&load_progress(state_dir)) {
            Some((_, e)) => PathBuf::from(&e.path),
            None => return Ok(Outcome::Fail(FccError::NoInput("no input files".into()))),
        },
    };
    Ok(Outcome::Run(path, config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Entry, Progress, save_progress};
    use std::fs;

    fn forbidden(text: &str) -> Vec<&'static str> {
        let low = text.to_lowercase();
        ["book", "reader", "okuyucu"]
            .into_iter()
            .filter(|w| low.contains(w))
            .collect()
    }

    fn setup() -> (tempfile::TempDir, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("a.txt");
        fs::write(&f, "hello").unwrap();
        (d, f)
    }

    fn run(argv: &[&str], dir: &Path) -> Outcome {
        resolve(argv.iter().copied(), dir).unwrap()
    }

    // -- test_cli.py --

    #[test]
    fn version_has_no_forbidden_words() {
        let (d, _) = setup();
        let e = resolve(["--version"], d.path()).unwrap_err();
        assert_eq!(e.kind(), clap::error::ErrorKind::DisplayVersion);
        let out = e.to_string();
        assert!(
            out.starts_with(&format!("larpr {} (", env!("CARGO_PKG_VERSION"))),
            "{out}"
        );
        assert!(forbidden(&out).is_empty());
    }

    #[test]
    fn help_has_no_forbidden_words() {
        let (d, _) = setup();
        let top = resolve(["--help"], d.path()).unwrap_err().to_string();
        assert!(top.contains("build"));
        assert!(forbidden(&top).is_empty(), "{top}");
        let sub = resolve(["build", "--help"], d.path())
            .unwrap_err()
            .to_string();
        assert!(sub.contains("--mask"));
        assert!(forbidden(&sub).is_empty(), "{sub}");
    }

    #[test]
    fn build_updates_config() {
        let (d, f) = setup();
        let f_s = f.to_str().unwrap();
        let out = run(
            &[
                "build",
                f_s,
                "--mask=code",
                "--lang=go",
                "--noise=4",
                "--auto",
                "--wpm=300",
            ],
            d.path(),
        );
        let Outcome::Run(path, config) = out else {
            panic!("{out:?}")
        };
        assert_eq!(path, f);
        assert_eq!(
            (
                config.mask.as_str(),
                config.lang.as_str(),
                config.noise,
                config.auto,
                config.wpm
            ),
            ("code", "go", 4, true, 300)
        );
        assert_eq!(load_config(d.path()), config);
    }

    #[test]
    fn theme_flag_is_saved() {
        let (d, f) = setup();
        let Outcome::Run(_, config) =
            run(&["build", f.to_str().unwrap(), "--theme=light"], d.path())
        else {
            panic!()
        };
        assert_eq!(config.theme, "light");
        assert_eq!(load_config(d.path()).theme, "light");
    }

    #[test]
    fn build_without_flags_keeps_saved_config() {
        let (d, f) = setup();
        let saved = Config {
            lang: "java".into(),
            noise: 1,
            ..Config::default()
        };
        save_config(d.path(), &saved).unwrap();
        let Outcome::Run(_, config) = run(&["build", f.to_str().unwrap()], d.path()) else {
            panic!()
        };
        assert_eq!((config.lang.as_str(), config.noise), ("java", 1));
    }

    #[test]
    fn no_args_uses_last_entry() {
        let (d, f) = setup();
        let e = |p: &Path, at: &str| Entry {
            path: p.to_string_lossy().into(),
            chapter: 0,
            paragraph: 0,
            bookmarks: vec![],
            updated_at: at.into(),
        };
        let mut data = Progress::new();
        data.insert(
            "x".into(),
            e(&d.path().join("old.txt"), "2026-01-01T00:00:00"),
        );
        data.insert("y".into(), e(&f, "2026-02-01T00:00:00"));
        save_progress(d.path(), &data).unwrap();
        let Outcome::Run(path, _) = run(&[], d.path()) else {
            panic!()
        };
        assert_eq!(path, f);
    }

    #[test]
    fn no_args_without_history_errors() {
        let (d, _) = setup();
        let out = run(&[], d.path());
        assert_eq!(
            out,
            Outcome::Fail(FccError::NoInput("no input files".into()))
        );
    }

    #[test]
    fn bad_flag_rejected_before_config_saved() {
        let (d, f) = setup();
        let f_s = f.to_str().unwrap();
        for flag in [
            "--noise=9",
            "--noise=-1",
            "--wpm=0",
            "--wpm=59",
            "--wpm=1501",
            "--wpm=abc",
            "--wpm=2.5",
            "--mask=nope",
            "--lang=cobol",
            "--hide=x",
        ] {
            let e = resolve(["build", f_s, flag], d.path()).unwrap_err();
            assert_eq!(e.exit_code(), 2, "{flag}");
            assert!(!d.path().join("config.json").exists(), "{flag}");
        }
    }

    #[test]
    fn noise_9_does_not_poison_later_runs() {
        let (d, f) = setup();
        let f_s = f.to_str().unwrap();
        assert!(resolve(["build", f_s, "--noise", "9"], d.path()).is_err());
        let Outcome::Run(_, config) = run(&["build", f_s], d.path()) else {
            panic!()
        };
        assert_eq!(config.noise, Config::default().noise);
    }
}
