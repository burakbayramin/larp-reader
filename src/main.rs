use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use larp_reader::cli::{Outcome, Pick, resolve};
use larp_reader::find::{choose, describe_roots, home_dir, listing};
use larp_reader::ui::app::App;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

/// Longest wait for input before timers are checked again.
const MAX_WAIT: Duration = Duration::from_millis(250);
/// Most files the picker lists.
const PICK_LIMIT: usize = 30;

fn run(app: &mut App, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        if app.should_quit() {
            return Ok(());
        }
        let wait = app
            .next_deadline()
            .saturating_duration_since(Instant::now())
            .min(MAX_WAIT);
        if event::poll(wait)? {
            match event::read()? {
                // Windows reports releases too; act on presses only
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    app.handle_key(k, Instant::now())
                }
                Event::Resize(w, h) => app.resize(w, h),
                _ => {}
            }
        }
        app.on_time(Instant::now());
    }
}

/// `larpr export`: the marks as Markdown on stdout, or in `out`.
fn export(path: &Path, out: Option<&Path>, state_dir: &Path) -> ExitCode {
    let markdown = match larp_reader::export::export(path, state_dir) {
        Ok(m) => m,
        Err(err) => {
            eprintln!("error: {}\n{err}", err.title());
            return ExitCode::from(1);
        }
    };
    let written = match out {
        Some(out) => std::fs::write(out, markdown),
        None => io::stdout().write_all(markdown.as_bytes()),
    };
    match written {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: cannot write output: {}", e.kind());
            ExitCode::from(1)
        }
    }
}

/// List the search's files in the plain terminal and, when someone is at it, ask which to
/// open: `Ok` is what opening the pick means, `Err` the status to exit with.
fn pick(pick: Pick, state_dir: &Path) -> Result<Outcome, ExitCode> {
    let home = home_dir();
    let search = &pick.search;
    let mut found = search.run(PICK_LIMIT + 1);
    let more = found.len() > PICK_LIMIT;
    found.truncate(PICK_LIMIT);
    if found.is_empty() {
        let folder = if cfg!(windows) {
            r"C:\Users\me\Downloads"
        } else {
            "~/Downloads"
        };
        let sep = std::path::MAIN_SEPARATOR;
        match &search.query {
            Some(q) => eprintln!("no supported files with \"{q}\" in the name found."),
            None => eprintln!("no supported files found."),
        }
        eprintln!(
            "searched: {}",
            describe_roots(&search.roots, home.as_deref())
        );
        if search.query.is_none() && search.roots.iter().all(|r| !r.explicit) {
            eprintln!("(.txt, .md and .html files are listed only with a name to match or --dir)");
        }
        eprintln!();
        eprintln!("give the path:     larpr build \"{folder}{sep}file.epub\"");
        eprintln!("or list a folder:  larpr find --dir \"{folder}\"");
        eprintln!(
            "tip: drag a file into the terminal to paste its path; keep the quotes if it has spaces"
        );
        return Err(ExitCode::from(1));
    }
    let mut out = io::stdout().lock();
    let printed = (|| -> io::Result<()> {
        for line in listing(&found, home.as_deref()) {
            writeln!(out, "{line}")?;
        }
        if more {
            writeln!(
                out,
                "(the {PICK_LIMIT} newest; add part of the name to narrow it: larpr find NAME)"
            )?;
        }
        Ok(())
    })();
    if printed.is_err() || !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        return Err(ExitCode::SUCCESS); // only the list when piped (or the pipe closed)
    }
    match choose(&mut io::stdin().lock(), &mut out, found.len()) {
        Ok(Some(i)) => {
            drop(out);
            Ok(pick.open(found.swap_remove(i).path, state_dir))
        }
        Ok(None) => Err(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("error: cannot read the answer: {}", e.kind());
            Err(ExitCode::from(1))
        }
    }
}

fn main() -> ExitCode {
    let state_dir = match larp_reader::state::state_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot create state folder: {}", e.kind());
            return ExitCode::from(1);
        }
    };
    let mut outcome = match resolve(std::env::args_os().skip(1), &state_dir) {
        Err(e) => e.exit(), // --help, --version, or a bad flag (status 2)
        Ok(o) => o,
    };
    // the picker runs in the plain terminal, before the full-screen app starts
    if let Outcome::Pick(p) = outcome {
        outcome = match pick(p, &state_dir) {
            Ok(o) => o,
            Err(code) => return code,
        };
    }
    let (path, config) = match outcome {
        Outcome::Fail(err) => {
            eprintln!("error: {}\n{err}", err.title());
            return ExitCode::from(1);
        }
        Outcome::Print(line) => {
            println!("{line}");
            return ExitCode::SUCCESS;
        }
        Outcome::Run(path, config) => (path, config),
        Outcome::Export(path, out) => return export(&path, out.as_deref(), &state_dir),
        Outcome::Pick(_) => unreachable!("opening a pick never asks for another"),
    };

    let theme = larp_reader::ui::theme::Theme::new(
        &config.theme,
        larp_reader::ui::theme::detect_depth_from_env(),
    );
    let mut terminal = ratatui::init(); // restores the terminal on a real panic
    let restore_and_report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !larp_reader::panic_is_quiet() {
            restore_and_report(info);
        }
    }));
    let size = terminal.size().unwrap_or_default();
    let mut app = App::new(
        Some(path),
        config,
        theme,
        state_dir,
        size.width,
        size.height,
        Instant::now(),
    );
    let result = run(&mut app, &mut terminal);
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: terminal: {}", e.kind());
            ExitCode::from(1)
        }
    }
}
