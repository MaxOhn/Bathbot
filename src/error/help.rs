#[derive(Debug, thiserror::Error)]
pub enum InvalidHelpState {
    #[error("unknown command")]
    UnknownCommand,
    #[error("missing embed")]
    MissingEmbed,
    #[error("missing embed title")]
    MissingTitle,
}
