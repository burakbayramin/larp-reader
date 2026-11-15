//! fcc: the reading engine and its terminal front end.
//!
//! Ported from the Python version layer by layer; see
//! `docs/superpowers/plans/2026-11-11-rust-port.md` for the map and order.

pub mod book;
pub mod cli;
pub mod errors;
pub mod loaders;
pub mod masks;
pub mod noise;
pub mod pace;
pub mod session;
pub mod state;
pub mod text;
pub mod ui;

thread_local! {
    static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f`, turning a panic inside it into an `Err` without printing anything.
///
/// For parsers of untrusted files that may panic on malformed input: the
/// screen must not be scribbled over, and the file just counts as unreadable.
pub fn quietly<R>(f: impl FnOnce() -> R + std::panic::UnwindSafe) -> std::thread::Result<R> {
    QUIET.with(|q| q.set(true));
    let result = std::panic::catch_unwind(f);
    QUIET.with(|q| q.set(false));
    result
}

/// Whether a panic happening now is one `quietly` expects.
pub fn panic_is_quiet() -> bool {
    QUIET.with(|q| q.get())
}

/// `fcc --version`: looks like a compiler's, never names what the program really is.
pub fn version() -> String {
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc".to_string()
    } else {
        format!("{}-unknown-linux-gnu", std::env::consts::ARCH)
    };
    format!("fcc {} ({target})", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_looks_like_a_build_tool() {
        let v = version();
        assert!(v.starts_with(&format!("fcc {} (", env!("CARGO_PKG_VERSION"))));
        for word in ["book", "reader", "okuyucu"] {
            assert!(!v.to_lowercase().contains(word));
        }
    }
}
