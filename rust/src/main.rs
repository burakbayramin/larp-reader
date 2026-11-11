use std::process::ExitCode;

use fcc::cli::{Outcome, resolve};

fn main() -> ExitCode {
    let state_dir = match fcc::state::state_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error: cannot create state folder: {}", e.kind());
            return ExitCode::from(1);
        }
    };
    match resolve(std::env::args_os().skip(1), &state_dir) {
        Err(e) => e.exit(), // --help, --version, or a bad flag (status 2)
        Ok(Outcome::Fail(err)) => {
            eprintln!("error: {}\n{err}", err.title());
            ExitCode::from(1)
        }
        Ok(Outcome::Run(_path, _config)) => {
            // the terminal interface is ported in stage 6
            eprintln!("error: interactive mode is not available in this build yet");
            ExitCode::from(1)
        }
    }
}
