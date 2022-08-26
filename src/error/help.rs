#[derive(Debug, thiserror::Error)]
pub enum InvalidHelpState {
    #[error("unknown command")]
    UnknownCommand,
    #[error("missing embed")]
    MissingEmbed,
    #[error("missing embed title")]
    MissingTitle,
    #[error("missing menu value")]
    MissingValue,
    #[error("got unexpected value `{0}`")]
    UnknownValue(String),
}
