use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusKind {
    Success,
    ClientError,
    ServerError,
    UnknownError,
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

    pub fn unknown_error(code: u32) -> Self {
        Self::new(StatusKind::UnknownError, code)
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
            Self::Success => write!(f, "OK"),
            Self::ClientError => write!(f, "CE"),
            Self::ServerError => write!(f, "SE"),
            Self::UnknownError => write!(f, "UE"),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.kind, self.code)
    }
}
