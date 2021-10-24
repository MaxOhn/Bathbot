use thiserror::Error;

#[derive(Debug, Error)]
pub enum PPError {
    #[error("io error")]
    IoError(#[from] tokio::io::Error),
    #[error("missing map id")]
    NoMapId,
    #[error("error while parsing beatmap file")]
    Parse(#[from] rosu_pp::ParseError),
}
