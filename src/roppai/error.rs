use std::{error::Error, fmt};

#[derive(Debug)]
pub enum OppaiErr {
    Binding(String),
    More(String),
    Syntax(String),
    Truncated(String),
    NotImplemented(String),
    IO(String),
    Format(String),
    OOM(String),
    UnexpectedCode(String),
    MissingPath(String),
}

impl fmt::Display for OppaiErr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Oppai ")?;
        match self {
            Self::Binding(e) => write!(f, "binding: {}", e),
            Self::More(e) => write!(f, "more: {}", e),
            Self::Syntax(e) => write!(f, "syntax: {}", e),
            Self::Truncated(e) => write!(f, "truncated: {}", e),
            Self::NotImplemented(e) => write!(f, "not implemented: {}", e),
            Self::IO(e) => write!(f, "io: {}", e),
            Self::Format(e) => write!(f, "format: {}", e),
            Self::OOM(e) => write!(f, "oom: {}", e),
            Self::UnexpectedCode(e) => write!(f, "unexpected code: {}", e),
            Self::MissingPath(e) => write!(f, "missing path: {}", e),
        }
    }
}

impl OppaiErr {
    pub(crate) fn new(code: i32, msg: impl AsRef<str>) -> Self {
        let msg = String::from(msg.as_ref());
        match code {
            -1 => OppaiErr::More(msg),
            -2 => OppaiErr::Syntax(msg),
            -3 => OppaiErr::Truncated(msg),
            -4 => OppaiErr::NotImplemented(msg),
            -5 => OppaiErr::IO(msg),
            -6 => OppaiErr::Format(msg),
            -7 => OppaiErr::OOM(msg),
            _ => OppaiErr::UnexpectedCode(format!(
                "Expected error codes -1 to -7, got {}: {}",
                code, msg
            )),
        }
    }
}

impl Error for OppaiErr {}
