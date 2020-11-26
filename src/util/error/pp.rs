use crate::pp::roppai::OppaiErr;

use rosu::model::GameMode;
use std::{error::Error as StdError, fmt};
use tokio::io::Error as IoError;

#[derive(Debug)]
pub enum PPError {
    CommandLine(String),
    InvalidFloat(String),
    IoError(IoError),
    MaxPP(Box<PPError>),
    NoContext(GameMode),
    NoMapId,
    NoScore,
    Oppai(OppaiErr),
    Output(String),
    PP(Box<PPError>),
    Stars(Box<PPError>),
    Timeout,
}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CommandLine(_) => f.write_str("command line error"),
            Self::InvalidFloat(e) => write!(f, "could not parse float: {}", e),
            Self::IoError(_) => f.write_str("io error"),
            Self::MaxPP(_) => f.write_str("error for max pp"),
            Self::NoContext(m) => write!(f, "missing context for {:?}", m),
            Self::NoMapId => f.write_str("missing map id"),
            Self::NoScore => f.write_str("missing score"),
            Self::Oppai(_) => f.write_str("error while using oppai"),
            Self::Output(e) => write!(f, "output error: {}", e),
            Self::PP(_) => f.write_str("error for pp"),
            Self::Stars(_) => f.write_str("error for stars"),
            Self::Timeout => f.write_str("calculation took too long, timed out"),
        }
    }
}

impl From<OppaiErr> for PPError {
    fn from(e: OppaiErr) -> Self {
        Self::Oppai(e)
    }
}

impl StdError for PPError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::CommandLine(_) => None,
            Self::InvalidFloat(_) => None,
            Self::IoError(e) => Some(e),
            Self::MaxPP(e) => Some(e),
            Self::NoContext(_) => None,
            Self::NoMapId => None,
            Self::NoScore => None,
            Self::Oppai(e) => Some(e),
            Self::Output(_) => None,
            Self::PP(e) => Some(e),
            Self::Stars(e) => Some(e),
            Self::Timeout => None,
        }
    }
}
