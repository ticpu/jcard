//! Error types for jCard parsing.

use std::fmt;

/// Error returned by jCard parsing operations.
///
/// [`InvalidJson`](Self::InvalidJson) indicates the input is not valid JSON.
/// [`InvalidStructure`](Self::InvalidStructure) indicates valid JSON that is
/// not a jCard (e.g., missing `"vcard"` tag, not an array).
#[derive(Debug)]
pub enum Error {
    /// The input is not valid JSON.
    InvalidJson(Box<dyn std::error::Error + Send + Sync>),
    /// Valid JSON but not a valid jCard structure.
    InvalidStructure(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(e) => write!(f, "invalid JSON: {e}"),
            Self::InvalidStructure(msg) => write!(f, "invalid jCard structure: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidJson(e) => Some(&**e),
            Self::InvalidStructure(_) => None,
        }
    }
}
