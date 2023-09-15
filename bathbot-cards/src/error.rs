use std::io::Error as IoError;

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CardError {
    #[error("Failed to create surface")]
    CreateSurface,
    #[error("Failed to encode as PNG")]
    EncodeAsPng,
    #[error("Failed to draw background")]
    Background(#[from] BackgroundError),
    #[error("Failed to draw header")]
    Header(#[from] HeaderError),
    #[error("Failed to draw info")]
    Info(#[from] InfoError),
    #[error("Failed to draw footer")]
    Footer(#[from] FooterError),
}

#[derive(Debug, ThisError)]
pub enum PaintError {
    #[error("Failed to create gradient")]
    Gradient,
    #[error("Failed to create mask filter")]
    MaskFilter,
}

#[derive(Debug, ThisError)]
pub enum FontError {
    #[error("Failed to create typeface")]
    Typeface,
}

#[derive(Debug, ThisError)]
pub enum BackgroundError {
    #[error("Failed to create image")]
    Image,
    #[error("IO error")]
    Io(#[from] IoError),
}

#[derive(Debug, ThisError)]
pub enum HeaderError {
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Failed to create image")]
    Flag,
    #[error("Failed to read mode file")]
    ModeFile(#[source] IoError),
    #[error("Failed to parse mode path")]
    ModePath,
    #[error("Paint error")]
    Paint(#[from] PaintError),
}

#[derive(Debug, ThisError)]
pub enum InfoError {
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Paint error")]
    Paint(#[from] PaintError),
}

#[derive(Debug, ThisError)]
pub enum FooterError {
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Paint error")]
    Paint(#[from] PaintError),
}

#[derive(Debug, ThisError)]
pub enum SvgError {
    #[error("Failed to create path")]
    CreatePath,
    #[error("Missing svg path")]
    MissingPath,
    #[error("Missing svg viewBox")]
    MissingViewBox,
    #[error("Missing svg viewBox height")]
    MissingViewBoxH,
    #[error("Missing svg viewBox width")]
    MissingViewBoxW,
    #[error("Failed to parse svg viewBox")]
    ParseViewBox,
}
