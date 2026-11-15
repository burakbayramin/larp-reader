use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use larp_reader::cli::{Outcome, resolve};
use larp_reader::ui::app::App;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

/// Longest wait for input before timers are checked again.
const MAX_WAIT: Duration = Duration::from_millis(250);

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

fn main() -> ExitCode {
    let state_dir = match larp_reader::state::state_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot create state folder: {}", e.kind());
            return ExitCode::from(1);
        }
    };
    let (path, config) = match resolve(std::env::args_os().skip(1), &state_dir) {
        Err(e) => e.exit(), // --help, --version, or a bad flag (status 2)
        Ok(Outcome::Fail(err)) => {
            eprintln!("error: {}\n{err}", err.title());
            return ExitCode::from(1);
        }
        Ok(Outcome::Run(path, config)) => (path, config),
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
