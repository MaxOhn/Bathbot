use std::{io::Error as IoError, str::Utf8Error};

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum CardError {
    #[error("Failed to read font file `{path}`")]
    LoadFont {
        #[source]
        source: IoError,
        path: Box<str>,
    },
    #[error("Failed to create surface")]
    CreateSurface,
    #[error("Failed to draw background")]
    Background(#[from] BackgroundError),
    #[error("Failed to draw header")]
    Header(#[from] HeaderError),
    #[error("Failed to draw info")]
    Info(#[from] InfoError),
    #[error("Failed to draw footer")]
    Footer(#[from] FooterError),
    #[error("Failed to encode as PNG")]
    EncodeAsPng,
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
    #[error("Failed to read image file")]
    File(#[source] IoError),
}

#[derive(Debug, ThisError)]
pub enum HeaderError {
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Failed to create flag image")]
    Flag,
    #[error("Failed to read mode file")]
    ModeFile(#[source] IoError),
    #[error("Failed to parse mode svg")]
    ModeSvg(SvgError),
    #[error("Paint error")]
    Paint(#[from] PaintError),
    #[error("Failed to make title text blob")]
    TitleTextBlob,
}

#[derive(Debug, ThisError)]
pub enum InfoError {
    #[error("Failed to create avatar image")]
    Avatar,
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Paint error")]
    Paint(#[from] PaintError),
    #[error("Failed to make skill text blob")]
    SkillTextBlob,
}

#[derive(Debug, ThisError)]
pub enum FooterError {
    #[error("Font error")]
    Font(#[from] FontError),
    #[error("Failed to create icon image")]
    Icon,
    #[error("Failed to read logo file")]
    LogoFile(#[source] IoError),
    #[error("Failed to read branding file")]
    BrandingFile(#[source] IoError),
    #[error("Paint error")]
    Paint(#[from] PaintError),
    #[error("Failed to parse branding svg")]
    BrandingSvg(#[source] SvgError),
}

#[derive(Debug, ThisError)]
pub enum SvgError {
    #[error("Failed to create path")]
    CreatePath,
    #[error("Missing path")]
    MissingPath,
    #[error("Missing viewBox")]
    MissingViewBox,
    #[error("Missing viewBox height")]
    MissingViewBoxH,
    #[error("Missing viewBox width")]
    MissingViewBoxW,
    #[error("Failed to parse viewBox")]
    ParseViewBox,
    #[error("Failed UTF-8 validation")]
    Utf8(#[from] Utf8Error),
}
