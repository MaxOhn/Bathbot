use rosu_pp::ParseError;
use std::{error::Error as StdError, fmt};
use tokio::io::Error as IoError;

#[derive(Debug)]
pub enum PPError {
    IoError(IoError),
    NoMapId,
    Parse(ParseError),
}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::IoError(_) => f.write_str("io error"),
            Self::NoMapId => f.write_str("missing map id"),
            Self::Parse(_) => f.write_str("error while parsing beatmap file"),
        }
    }
}

impl From<IoError> for PPError {
    fn from(e: IoError) -> Self {
        Self::IoError(e)
    }
}

impl From<ParseError> for PPError {
    fn from(e: ParseError) -> Self {
        Self::Parse(e)
    }
}

impl StdError for PPError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            Self::NoMapId => None,
            Self::Parse(e) => Some(e),
        }
    }
}
