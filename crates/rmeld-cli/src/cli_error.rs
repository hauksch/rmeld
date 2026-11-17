//! CLI-side error wrapper: reuses the library's exit-code mapping and adds
//! the usage-level variants the CLI itself introduces.

use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    /// The library's own errors keep their exit codes (fachlich verdicts,
    /// validation failures, internal plumbing).
    Rmeld(rmeld::Error),
    /// Input contract violations on the CLI boundary (bad JSON, unreadable
    /// file) — usage, exit 2.
    InvalidInput(String),
}

impl Error {
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Rmeld(e) => e.exit_code(),
            Error::InvalidInput(_) => 2,
        }
    }

    pub fn from_rmeld(e: rmeld::Error) -> Error {
        Error::Rmeld(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Rmeld(e) => e.fmt(f),
            Error::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Rmeld(rmeld::Error::Io(e))
    }
}

/// Silence the unused-path warning until a CLI flag needs it; the type is
/// part of the stable surface.
#[allow(dead_code)]
fn _type_surface(_: PathBuf) {}

impl From<rmeld::Error> for Error {
    fn from(e: rmeld::Error) -> Self {
        Error::Rmeld(e)
    }
}
