//! Errors shown to the user inside the active mask (`errors.py`).
//!
//! `title()` is the mask's headline and `Display` the description. Neither
//! may contain the input's file name: it would give away what is being read.

use std::io;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FccError {
    #[error("{0}")]
    UnsupportedFormat(String),
    #[error("{0}")]
    DrmProtected(String),
    #[error("{0}")]
    NoTextLayer(String),
    #[error("{0}")]
    EmptyBook(String),
    #[error("{0}")]
    NoInput(String),
    #[error("{0}")]
    InputUnavailable(String),
    /// A setting to fix before starting, e.g. a bad `keys` object.
    #[error("{0}")]
    Config(String),
    /// Anything else, e.g. "could not save state".
    #[error("{0}")]
    Other(String),
}

impl FccError {
    pub fn title(&self) -> &'static str {
        match self {
            FccError::UnsupportedFormat(_) => "unsupported input format",
            FccError::DrmProtected(_) => "DRM-protected input",
            FccError::NoTextLayer(_) => "no text layer found",
            FccError::EmptyBook(_) => "empty input",
            FccError::NoInput(_) => "no input files",
            FccError::InputUnavailable(_) => "cannot open input",
            FccError::Config(_) => "invalid config",
            FccError::Other(_) => "error",
        }
    }

    /// Describe an I/O error without its file name (which would reveal the title).
    pub fn from_io(e: &io::Error) -> FccError {
        if e.kind() == io::ErrorKind::NotFound {
            FccError::InputUnavailable("input file not found".into())
        } else {
            // the kind's description, never the OS message, which some platforms
            // build with the path in it
            FccError::InputUnavailable(format!("input file unreadable: {}", e.kind()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_messages_are_separate() {
        let e = FccError::NoTextLayer("scanned pages, OCR is not supported".into());
        assert_eq!(e.title(), "no text layer found");
        assert_eq!(e.to_string(), "scanned pages, OCR is not supported");
    }

    #[test]
    fn io_errors_never_carry_the_path() {
        let path = "/home/x/Suç ve Ceza.epub";
        let missing = io::Error::new(io::ErrorKind::NotFound, format!("{path}: no such file"));
        let denied = io::Error::new(io::ErrorKind::PermissionDenied, format!("{path}: denied"));
        assert_eq!(
            FccError::from_io(&missing).to_string(),
            "input file not found"
        );
        let msg = FccError::from_io(&denied).to_string();
        assert!(msg.starts_with("input file unreadable: "));
        assert!(!msg.contains("Suç"));
    }
}
