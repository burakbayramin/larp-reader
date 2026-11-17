//! Command line (`cli.py`): `larpr build [FILE] [--mask …] [--lang …] [--noise …] [--auto] [--wpm …] [--hide …]`,
//! `larpr find [QUERY] [--dir DIR]`. A directory as FILE, or no FILE and no history,
//! turns into a pick from a list (`Outcome::Pick`) before the app starts.
//!
//! Like `--version`, `--help` reads like a build tool's and never says what the
//! program really does.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::builder::PossibleValuesParser;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};

use crate::errors::FccError;
use crate::find::{Root, Search};
use crate::masks::{LANGS, MASK_NAMES};
use crate::noise::MAX_NOISE;
use crate::pace::{MAX_WPM, MIN_WPM};
use crate::state::{
    Config, HIDE_STYLES, THEMES, last_entry, load_config, load_progress, save_config,
    save_config_without_theme, theme_is_saved,
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
    /// print one status line
    Tick(TickArgs),
    /// write saved marks as Markdown
    Export(ExportArgs),
    /// list inputs
    Find(FindArgs),
}

#[derive(Args, Debug)]
struct FindArgs {
    /// only inputs whose name contains this
    query: Option<String>,
    /// search this directory instead of the default paths
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct TickArgs {
    /// widest the line may be, in columns
    #[arg(long, default_value_t = crate::tick::DEFAULT_WIDTH)]
    width: usize,
}

#[derive(Args, Debug, Default)]
struct ExportArgs {
    /// input file (default: last used)
    file: Option<PathBuf>,
    /// write here instead of stdout
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,
}

#[derive(Args, Debug, Default, Clone, PartialEq, Eq)]
struct BuildArgs {
    /// input file or directory (default: last used)
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
    /// Print this one line and exit with status 0.
    Print(String),
    /// Write this file's saved marks as Markdown, to stdout or to the path.
    Export(PathBuf, Option<PathBuf>),
    /// List the files this search finds; the one picked opens with `Pick::open`.
    Pick(Pick),
}

/// A search to list in the plain terminal, and the `build` flags to open the pick with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub search: Search,
    build: BuildArgs,
    light_terminal: bool,
}

impl Pick {
    /// What `larpr build PATH` with the same flags would do. The flags are applied (and
    /// saved) only now, so cancelling the pick leaves the config alone.
    pub fn open(self, path: PathBuf, state_dir: &Path) -> Outcome {
        let args = BuildArgs {
            file: Some(path),
            ..self.build
        };
        build_outcome(args, state_dir, self.light_terminal)
    }
}

/// Parse `argv` (without the program name), apply flags to the saved config, save it and
/// pick the file to open. Bad flags come back as a clap error before anything is saved.
pub fn resolve<I, T>(argv: I, state_dir: &Path) -> Result<Outcome, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    resolve_on(
        argv,
        state_dir,
        crate::ui::theme::light_background_from_env(),
    )
}

/// `resolve`, told whether the terminal background is light: with no `--theme` and no
/// saved theme, a light terminal gets the light theme (and it is not saved).
fn resolve_on<I, T>(argv: I, state_dir: &Path, light_terminal: bool) -> Result<Outcome, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let argv = std::iter::once(OsString::from("larpr")).chain(argv.into_iter().map(Into::into));
    let matches = command().try_get_matches_from(argv)?;
    let cli = Cli::from_arg_matches(&matches)?;
    let args = match cli.cmd {
        Some(Command::Build(a)) => a,
        Some(Command::Tick(t)) => return Ok(Outcome::Print(crate::tick::tick(state_dir, t.width))),
        Some(Command::Export(a)) => return Ok(export_outcome(a, state_dir)),
        Some(Command::Find(a)) => return Ok(find_outcome(a, light_terminal)),
        None => BuildArgs::default(),
    };

    let pick = |search| {
        Ok(Outcome::Pick(Pick {
            search,
            build: BuildArgs {
                file: None,
                ..args.clone()
            },
            light_terminal,
        }))
    };
    match &args.file {
        Some(dir) if dir.is_dir() => {
            return pick(Search {
                roots: vec![Root::explicit(dir)],
                query: None,
            });
        }
        None if last_entry(&load_progress(state_dir)).is_none() => {
            return pick(Search::usual(None));
        }
        _ => {}
    }
    Ok(build_outcome(args, state_dir, light_terminal))
}

/// `larpr find [QUERY] [--dir DIR]`: a pick that opens with the saved config.
fn find_outcome(args: FindArgs, light_terminal: bool) -> Outcome {
    let search = match args.dir {
        Some(dir) if !dir.is_dir() => {
            return Outcome::Fail(FccError::NoInput("--dir is not a directory".into()));
        }
        Some(dir) => Search {
            roots: vec![Root::explicit(dir)],
            query: args.query,
        },
        None => Search::usual(args.query),
    };
    Outcome::Pick(Pick {
        search,
        build: BuildArgs::default(),
        light_terminal,
    })
}

/// `larpr build`: apply the flags to the saved config, save it, pick the file.
fn build_outcome(args: BuildArgs, state_dir: &Path, light_terminal: bool) -> Outcome {
    let mut config = load_config(state_dir);
    if let Err(e) = crate::keymap::Keymap::parse(&config.keys) {
        return Outcome::Fail(FccError::Config(e));
    }
    let auto_theme = args.theme.is_none() && !theme_is_saved(state_dir);
    if auto_theme && light_terminal {
        config.theme = "light".into();
    }
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
    let _ = if auto_theme {
        save_config_without_theme(state_dir, &config)
    } else {
        save_config(state_dir, &config)
    }; // best effort: the flags still apply to this run

    let path = match args.file {
        Some(p) => p,
        None => match last_entry(&load_progress(state_dir)) {
            Some((_, e)) => PathBuf::from(&e.path),
            None => return Outcome::Fail(FccError::NoInput("no input files".into())),
        },
    };
    Outcome::Run(path, config)
}

/// `larpr export`: the file to read (else the last one opened); the config is left alone.
fn export_outcome(args: ExportArgs, state_dir: &Path) -> Outcome {
    let path = match args.file {
        Some(p) => p,
        None => match last_entry(&load_progress(state_dir)) {
            Some((_, e)) => PathBuf::from(&e.path),
            None => return Outcome::Fail(FccError::NoInput("no input files".into())),
        },
    };
    Outcome::Export(path, args.out)
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
        resolve_on(argv.iter().copied(), dir, false).unwrap()
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

    fn theme_after(argv: &[&str], dir: &Path, light: bool) -> String {
        let Ok(Outcome::Run(_, config)) = resolve_on(argv.iter().copied(), dir, light) else {
            panic!()
        };
        config.theme
    }

    #[test]
    fn a_light_terminal_gets_the_light_theme_unsaved() {
        let (d, f) = setup();
        let file = f.to_str().unwrap();
        assert_eq!(theme_after(&["build", file], d.path(), true), "light");
        assert!(!theme_is_saved(d.path()));
        assert_eq!(load_config(d.path()).theme, "monokai");
        // the terminal turned dark again
        assert_eq!(theme_after(&["build", file], d.path(), false), "monokai");
    }

    #[test]
    fn a_dark_terminal_keeps_the_default_theme() {
        let (d, f) = setup();
        assert_eq!(
            theme_after(&["build", f.to_str().unwrap()], d.path(), false),
            "monokai"
        );
    }

    #[test]
    fn an_explicit_or_saved_theme_beats_the_terminal() {
        let (d, f) = setup();
        let file = f.to_str().unwrap();
        assert_eq!(
            theme_after(&["build", file, "--theme=dark"], d.path(), true),
            "dark"
        );
        assert!(theme_is_saved(d.path()));
        assert_eq!(theme_after(&["build", file], d.path(), true), "dark");
        let (d2, f2) = setup();
        let saved = Config {
            theme: "monokai".into(),
            ..Config::default()
        };
        save_config(d2.path(), &saved).unwrap();
        assert_eq!(
            theme_after(&["build", f2.to_str().unwrap()], d2.path(), true),
            "monokai"
        );
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
    fn no_args_without_history_searches_the_usual_folders() {
        let (d, f) = setup();
        for argv in [&[][..], &["build"], &["build", "--mask=code"]] {
            let Outcome::Pick(pick) = run(argv, d.path()) else {
                panic!("{argv:?}")
            };
            assert_eq!(pick.search, Search::usual(None), "{argv:?}");
        }
        assert!(!d.path().join("config.json").exists(), "nothing saved yet");
        let Outcome::Pick(pick) = run(&["build", "--mask=code"], d.path()) else {
            panic!()
        };
        let Outcome::Run(path, config) = pick.open(f.clone(), d.path()) else {
            panic!()
        };
        assert_eq!((path, config.mask.as_str()), (f, "code"));
        assert_eq!(load_config(d.path()).mask, "code");
    }

    #[test]
    fn a_directory_as_input_lists_its_files() {
        // a directory used to be handed to the app, which could only fail to open it
        let (d, f) = setup();
        let dir = d.path().to_str().unwrap();
        let Outcome::Pick(pick) = run(&["build", dir, "--wpm=300"], d.path()) else {
            panic!()
        };
        assert_eq!(
            pick.search,
            Search {
                roots: vec![Root::explicit(d.path())],
                query: None
            }
        );
        let Outcome::Run(path, config) = pick.open(f.clone(), d.path()) else {
            panic!()
        };
        assert_eq!((path, config.wpm), (f, 300));
    }

    #[test]
    fn find_lists_without_touching_config() {
        let (d, f) = setup();
        let Outcome::Pick(pick) = run(&["find", "dune"], d.path()) else {
            panic!()
        };
        assert_eq!(pick.search, Search::usual(Some("dune".into())));
        let dir = d.path().to_str().unwrap();
        let Outcome::Pick(pick) = run(&["find", "--dir", dir], d.path()) else {
            panic!()
        };
        assert_eq!(pick.search.roots, vec![Root::explicit(d.path())]);
        assert!(!d.path().join("config.json").exists());
        assert_eq!(pick.search.run(10)[0].path, f);
        let Outcome::Run(path, config) = pick.open(f.clone(), d.path()) else {
            panic!()
        };
        assert_eq!((path, config), (f.clone(), load_config(d.path())));
        assert_eq!(
            run(&["find", "--dir", f.to_str().unwrap()], d.path()),
            Outcome::Fail(FccError::NoInput("--dir is not a directory".into()))
        );
        let help = resolve(["find", "--help"], d.path())
            .unwrap_err()
            .to_string();
        assert!(help.contains("--dir"));
        assert!(forbidden(&help).is_empty(), "{help}");
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
    fn bad_keys_stop_the_build_with_a_clear_error() {
        let (d, f) = setup();
        let f_s = f.to_str().unwrap();
        fs::write(
            d.path().join("config.json"),
            r#"{"keys": {"bookmark": "m"}}"#,
        )
        .unwrap();
        let Outcome::Fail(FccError::Config(msg)) = run(&["build", f_s], d.path()) else {
            panic!()
        };
        assert!(
            msg.contains("bookmark") && msg.contains("next_view"),
            "{msg}"
        );
        fs::write(
            d.path().join("config.json"),
            r#"{"keys": {"bookmark": "B"}}"#,
        )
        .unwrap();
        let Outcome::Run(_, config) = run(&["build", f_s], d.path()) else {
            panic!()
        };
        assert_eq!(config.keys, serde_json::json!({"bookmark": "B"}));
        assert_eq!(load_config(d.path()).keys, config.keys, "saving keeps them");
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

    #[test]
    fn export_picks_the_file_and_out_without_touching_config() {
        let (d, f) = setup();
        let f_s = f.to_str().unwrap();
        assert_eq!(
            run(&["export", f_s], d.path()),
            Outcome::Export(f.clone(), None)
        );
        assert_eq!(
            run(&["export", f_s, "--out", "marks.md"], d.path()),
            Outcome::Export(f.clone(), Some(PathBuf::from("marks.md")))
        );
        assert!(!d.path().join("config.json").exists());
        assert_eq!(
            run(&["export"], d.path()),
            Outcome::Fail(FccError::NoInput("no input files".into()))
        );
        let help = resolve(["export", "--help"], d.path())
            .unwrap_err()
            .to_string();
        assert!(help.contains("--out"));
        assert!(forbidden(&help).is_empty(), "{help}");
    }
}
