//! Error type for the whole fachlich surface. House rule: fail-fast over
//! half-working states; no anyhow anywhere.

use std::path::PathBuf;

/// Errors from create, parse, classify and validate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The structured request input violates a fachlich invariant the XSD
    /// alone cannot express (e.g. Suchprofil needs an Anschrift or two
    /// further search criteria).
    #[error("fachlich invalid request: {0}")]
    InvalidRequest(&'static str),

    /// The XML is well-formed but not a message this crate understands.
    #[error("unknown message root element <{root}> in namespace {namespace}")]
    UnknownMessage {
        /// The root element's local name.
        root: String,
        /// The root element's namespace URI.
        namespace: String,
    },

    /// The XML is not well-formed or violates the wire structure.
    #[error("malformed xMeld message: {0}")]
    Malformed(String),

    /// xmllint reported schema violations; the text is its stderr verbatim.
    #[error("schema validation failed:\n{0}")]
    SchemaViolations(String),

    /// The xmllint binary is missing or unusable. Loud on purpose: a
    /// validation that silently skips is how local-green CI-red bugs are
    /// born (see rosci's history).
    #[error("xmllint not usable at {path:?}: {reason}")]
    XmllintUnavailable {
        /// Where xmllint was looked for.
        path: PathBuf,
        /// Why it was rejected (missing, non-zero `--version`, …).
        reason: String,
    },

    /// I/O or serialization plumbing beneath the fachlich layer.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// XML (de)serialization plumbing beneath the fachlich layer.
    #[error("xml: {0}")]
    Xml(String),
}

impl Error {
    /// Exit codes for the CLI, mirroring the rosci convention: 0 ok,
    /// 2 usage/config, 3..5 fachlich verdicts, 6 internal.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::InvalidRequest(_) => 2,
            Error::UnknownMessage { .. } => 2,
            Error::Malformed(_) => 2,
            Error::SchemaViolations(_) => 2,
            Error::XmllintUnavailable { .. } => 6,
            Error::Io(_) | Error::Xml(_) => 6,
        }
    }
}
