//! This module provides the iteration status for the benchmark.
use std::fmt;

/// Represents the kind of status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusKind {
    /// Indicates success status.
    Success,
    /// Indicates uncategorized error.
    Error,
    /// Indicates client error.
    ClientError,
    /// Indicates server error.
    ServerError,
}

/// The iteration status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Status {
    kind: StatusKind,
    code: u32,
}

impl Status {
    fn new(kind: StatusKind, code: u32) -> Self {
        Self { kind, code }
    }

    /// Creates a new success status.
    pub fn success(code: u32) -> Self {
        Self::new(StatusKind::Success, code)
    }

    /// Creates a new client error status.
    pub fn client_error(code: u32) -> Self {
        Self::new(StatusKind::ClientError, code)
    }

    /// Creates a new server error status.
    pub fn server_error(code: u32) -> Self {
        Self::new(StatusKind::ServerError, code)
    }

    /// Creates a new uncategorized error status.
    pub fn error(code: u32) -> Self {
        Self::new(StatusKind::Error, code)
    }

    /// Returns the kind of the status.
    pub fn kind(&self) -> StatusKind {
        self.kind
    }

    /// Returns the code of the status.
    pub fn code(&self) -> u32 {
        self.code
    }
}

impl fmt::Display for StatusKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Error => write!(f, "Error"),
            Self::ClientError => write!(f, "Client Error"),
            Self::ServerError => write!(f, "Server Error"),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.code)
    }
}
