use eyre::{Result, ContextCompat};
use image::DynamicImage;
use skia_safe::{Color, Surface, EncodedImageFormat};

pub enum GraphBackground<'a> {
    None,
    Color { color: Color },
    Image { image: &'a DynamicImage, dim: f32 },
}

pub enum GraphFill {
    #[allow(unused)]
    None,
    Solid(Color),
    Gradient(Color, Color),
}

pub struct GraphData {
    pub name: String,
    pub points: Vec<(f32, f32)>,
    pub color: Color,
    pub fill: GraphFill,
}

impl GraphData {
    pub fn new(name: String, points: Vec<(f32, f32)>, color: Color, fill: GraphFill) -> Self {
        Self { name, points, color, fill }
    }
}

pub struct Graph {
    pub surface: Surface,
}

impl Graph {
    pub fn new(surface: Surface) -> Self {
        Self {
            surface
        }
    }

    pub fn to_image_mut(&mut self, format: EncodedImageFormat) -> Result<Vec<u8>> {
        let data = self.surface.image_snapshot().encode_to_data(format).wrap_err("")?;
        Ok(data.as_bytes().to_vec())
    }
}

pub type GraphComponent = Result<(i32, Option<Surface>)>;
