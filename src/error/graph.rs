#[derive(Debug, thiserror::Error)]
#[error("failed to create graph")]
pub enum GraphError {
    #[error("client error")]
    Client(#[from] crate::custom_client::CustomClientError),
    #[error("failed to calculate curve")]
    Curve(#[from] enterpolation::linear::LinearError),
    #[error("image error")]
    Image(#[from] image::ImageError),
    #[error("no non-zero strain point")]
    InvalidStrainPoints,
    #[error("plotter error: {0}")]
    Plotter(String),
}
