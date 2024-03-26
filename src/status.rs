use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusKind {
    /// Indicates success status
    Success,
    /// Indicates uncategorized error
    Error,
    /// Indicates client error
    ClientError,
    /// Indicates server error
    ServerError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Status {
    kind: StatusKind,
    code: u32,
}

impl Status {
    pub fn new(kind: StatusKind, code: u32) -> Self {
        Self { kind, code }
    }

    pub fn success(code: u32) -> Self {
        Self::new(StatusKind::Success, code)
    }

    pub fn client_error(code: u32) -> Self {
        Self::new(StatusKind::ClientError, code)
    }

    pub fn server_error(code: u32) -> Self {
        Self::new(StatusKind::ServerError, code)
    }

    pub fn error(code: u32) -> Self {
        Self::new(StatusKind::Error, code)
    }

    pub fn kind(&self) -> StatusKind {
        self.kind
    }

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
