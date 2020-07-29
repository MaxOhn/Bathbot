use crate::pp::roppai::OppaiErr;

use rosu::models::GameMode;
use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum PPError {
    CommandLine(String),
    MaxPP(Box<PPError>),
    NoContext(GameMode),
    NoMapId,
    Oppai(OppaiErr),
    PP(Box<PPError>),
    Stars(Box<PPError>),
    Timeout,
}

impl fmt::Display for PPError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::CommandLine(e) => write!(f, "command line error: {}", e),
            Self::MaxPP(e) => write!(f, "error for max pp: {}", e),
            Self::NoContext(m) => write!(f, "missing context for {:?}", m),
            Self::NoMapId => f.write_str("missing map id"),
            Self::Oppai(e) => write!(f, "error while using oppai: {}", e),
            Self::PP(e) => write!(f, "error for pp: {}", e),
            Self::Stars(e) => write!(f, "error for stars: {}", e),
            Self::Timeout => f.write_str("calculation took too long, timed out"),
        }
    }
}

impl From<OppaiErr> for PPError {
    fn from(e: OppaiErr) -> Self {
        Self::Oppai(e)
    }
}

impl StdError for PPError {}
