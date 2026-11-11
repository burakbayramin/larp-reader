//! fcc: the reading engine and its terminal front end.
//!
//! Ported from the Python version layer by layer; see
//! `docs/superpowers/plans/2026-11-11-rust-port.md` for the map and order.

pub mod book;
pub mod errors;
pub mod text;

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
        assert!(v.starts_with("fcc 0.4.2 ("));
        for word in ["book", "reader", "okuyucu"] {
            assert!(!v.to_lowercase().contains(word));
        }
    }
}
