use eyre::Report;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("status code 400 - bad request")]
    BadRequest,
    #[error("status code 404 - not found")]
    NotFound,
    #[error("status code 429 - ratelimited")]
    Ratelimited,
    #[error(transparent)]
    Report(#[from] Report),
}
