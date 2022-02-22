#[derive(Debug, thiserror::Error)]
pub enum MapFileError {
    #[error("client error")]
    Client(#[from] crate::custom_client::CustomClientError),
    #[error("io error")]
    CreateFile(#[from] std::io::Error),
}
