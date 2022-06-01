#[derive(Debug, thiserror::Error)]
pub enum InvalidModal {
    #[error("missing input for page number")]
    MissingPageInput,
}
