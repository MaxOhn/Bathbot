use chrono::format::ParseError as ParseChrono;
use diesel::result::Error as DieselError;
use image::ImageError;
use reqwest;
use roppai::OppaiErr;
use rosu::backend::OsuError;
use serde_json::error::Error as SerdeError;
use serenity::{framework::standard::CommandError, Error as SerenityError};
use std::{env, fmt, io, num};

#[derive(Debug)]
pub enum Error {
    Custom(String),
    Command(CommandError),
    ParseInt(num::ParseIntError),
    ParseChrono(ParseChrono),
    Io(io::Error),
    Serenity(SerenityError),
    Env(env::VarError),
    Osu(OsuError),
    Oppai(OppaiErr),
    Reqwest(reqwest::Error),
    DieselError(DieselError),
    MySQLConnection(String),
    ImageError(ImageError),
    Serde(SerdeError),
}

impl From<SerdeError> for Error {
    fn from(e: SerdeError) -> Self {
        Self::Serde(e)
    }
}

impl From<ImageError> for Error {
    fn from(e: ImageError) -> Self {
        Self::ImageError(e)
    }
}

impl From<DieselError> for Error {
    fn from(e: DieselError) -> Self {
        Self::DieselError(e)
    }
}

impl From<CommandError> for Error {
    fn from(e: CommandError) -> Self {
        Self::Command(e)
    }
}

impl From<num::ParseIntError> for Error {
    fn from(e: num::ParseIntError) -> Self {
        Self::ParseInt(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serenity::Error> for Error {
    fn from(e: SerenityError) -> Self {
        Self::Serenity(e)
    }
}

impl From<env::VarError> for Error {
    fn from(e: env::VarError) -> Self {
        Self::Env(e)
    }
}

impl From<OsuError> for Error {
    fn from(e: OsuError) -> Self {
        Self::Osu(e)
    }
}

impl From<ParseChrono> for Error {
    fn from(e: ParseChrono) -> Self {
        Self::ParseChrono(e)
    }
}

impl From<OppaiErr> for Error {
    fn from(e: OppaiErr) -> Self {
        Self::Oppai(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Custom(e) => write!(f, "{}", e),
            Self::Command(e) => write!(f, "{:?}", e),
            Self::ParseInt(e) => write!(f, "{}", e),
            Self::ParseChrono(e) => write!(f, "{}", e),
            Self::Io(e) => write!(f, "{}", e),
            Self::Serenity(e) => write!(f, "{}", e),
            Self::Env(e) => write!(f, "{}", e),
            Self::Osu(e) => write!(f, "{}", e),
            Self::Oppai(e) => write!(f, "{:?}", e),
            Self::Reqwest(e) => write!(f, "{:?}", e),
            Self::DieselError(e) => write!(f, "{}", e),
            Self::MySQLConnection(e) => write!(f, "{}", e),
            Self::ImageError(e) => write!(f, "{}", e),
            Self::Serde(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for Error {}
