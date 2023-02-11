use std::cmp::Ordering;

use eyre::{ContextCompat, Result, Context};
use itertools::Itertools;
use skia_safe::{Image, Typeface, Surface, PaintStyle, ImageInfo, ColorType, AlphaType, Data, Color, Paint, Rect, gradient_shader, TileMode, PaintCap, PaintJoin, BlendMode, Path, SamplingOptions, FilterMode, MipmapMode, TextBlob, Font};

use super::common::{GraphBackground, GraphData, GraphComponent, GraphFill, Graph};

pub struct LineGraphBuilder<'f> {
    size: (u32, u32),
    range: (std::ops::Range<f32>, std::ops::Range<f32>),
    draw_x_axis: bool,
    draw_y_axis: bool,
    draw_legend: bool,
    background: GraphBackground<'f>,
    line_width: f32,
    x_formatter: Option<&'f dyn Fn(f32) -> String>,
    y_formatter: Option<&'f dyn Fn(f32) -> String>,
    x_desc: Option<String>,
    y_desc: Option<String>,
    #[allow(unused)]
    top_image: Option<Image>,
    typeface: Typeface,
    graphs: Vec<GraphData>
}

impl<'f> LineGraphBuilder<'f> {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            size: (x, y),
            range: (0.0..0.0, 0.0..0.0),
            draw_x_axis: false,
            draw_y_axis: false,
            draw_legend: true,
            background: GraphBackground::None,
            line_width: 1.,
            x_formatter: None,
            y_formatter: None,
            x_desc: None,
            y_desc: None,
            top_image: None,
            typeface: Typeface::default(),
            graphs: Vec::new()
        }
    }

    pub fn set_range(&mut self, x: std::ops::Range<f32>, y: std::ops::Range<f32>) -> &mut Self {
        self.range = (x, y);
        self
    }

    pub fn set_draw_x_axis(&mut self, draw: bool) -> &mut Self {
        self.draw_x_axis = draw;
        self
    }

    pub fn set_draw_y_axis(&mut self, draw: bool) -> &mut Self {
        self.draw_y_axis = draw;
        self
    }

    pub fn set_draw_legend(&mut self, draw: bool) -> &mut Self {
        self.draw_legend = draw;
        self
    }

    pub fn set_background(&mut self, bg: GraphBackground<'f>) -> &mut Self {
        self.background = bg;
        self
    }

    pub fn set_line_width(&mut self, width: f32) -> &mut Self {
        self.line_width = width;
        self
    }

    pub fn set_x_formatter(&mut self, fmt: &'f dyn Fn(f32) -> String) -> &mut Self {
        self.x_formatter = Some(fmt);
        self
    }

    pub fn set_y_formatter(&mut self, fmt: &'f dyn Fn(f32) -> String) -> &mut Self {
        self.y_formatter = Some(fmt);
        self
    }

    #[allow(unused)]
    pub fn x_description(&mut self, desc: String) -> &mut Self {
        self.x_desc = Some(desc);
        self
    }

    #[allow(unused)]
    pub fn y_description(&mut self, desc: String) -> &mut Self {
        self.y_desc = Some(desc);
        self
    }

    pub fn set_typeface(&mut self, typeface: Typeface) -> &mut Self {
        self.typeface = typeface;
        self
    }

    pub fn add_graph(&mut self, graph: GraphData) -> &mut Self {
        self.graphs.push(graph);
        self
    }

    fn draw_legend(&self) -> GraphComponent {
        let mut surface = Surface::new_raster_n32_premul((self.size.0 as i32, 20)).wrap_err("")?;
        let canvas = surface.canvas();
        let mut paint = Paint::default();

        let mut x = 8.;

        let font = Font::new(&self.typeface, Some(14.0));

        for graph in self.graphs.iter() {
            paint
                .set_blend_mode(BlendMode::default())
                .set_color(graph.color.with_a(170));
            canvas.draw_rect(Rect::new(x, 7.5, x + 16., 12.5), &paint);

            x += 26.;

            paint.set_color(Color::WHITE);

            let textblob = TextBlob::from_str(graph.name.as_str(), &font).wrap_err("")?;
            canvas.draw_text_blob(&textblob, (x, 16.),  &paint);

            let (_, bounds) = font.measure_str(graph.name.as_str(), Some(&paint));

            x += bounds.width() + 10.0;
        }

        Ok((20, Some(surface)))
    }

    fn draw_graph(&self) -> GraphComponent {
        let mut surface = Surface::new_raster_n32_premul((self.size.0 as i32, self.size.1 as i32)).wrap_err("")?;
        let canvas = surface.canvas();
        let mut paint = Paint::default();
        paint
            .set_stroke_width(1.0)
            .set_stroke_cap(PaintCap::Round)
            .set_stroke_join(PaintJoin::Round)
            .set_anti_alias(true);

        let mut graph_area = Rect::new(0., 0., self.size.0 as f32, self.size.1 as f32);
        let (x_len, y_len) = (self.range.0.end - self.range.0.start, self.range.1.end - self.range.1.start);

        // if self.draw_y_axis {
        //     graph_area.left += 17.;
        // }
        // if self.draw_x_axis {
        //     graph_area.bottom -= 17.;
        // }

        if self.y_formatter.is_some() {
            graph_area.left += 19.;
        }
        if self.x_formatter.is_some() {
            graph_area.bottom -= 19.;
        }

        paint.set_color(Color::WHITE);

        let font = Font::new(&self.typeface, Some(12.0));

        if let Some(formatter) = self.x_formatter {
            let segments = f32::floor(graph_area.width() / 110.) as i32 + 1;
            for i in 1..segments {
                let k = i as f32 / segments as f32;

                let x = graph_area.left + graph_area.width() * k;

                paint.set_alpha_f(0.4);

                canvas.draw_line(
                    (x, graph_area.bottom),
                    (x, graph_area.top),
                    &paint
                );

                let label = formatter(k * x_len + self.range.0.start);
                let textblob = TextBlob::from_str(label.as_str(), &font).wrap_err("")?;

                let (_, bounds) = font.measure_str(label.as_str(), Some(&paint));
                let offset = bounds.width() / 2.;

                paint.set_alpha_f(1.);

                canvas.draw_text_blob(textblob, (x - offset, graph_area.bottom + 15.0), &paint);
            }
        }
        
        // TODO: Figure out how to properly rotate text

        if let Some(formatter) = self.y_formatter {
            let segments = f32::floor(graph_area.height() / 65.) as i32 + 1;
            for i in 1..segments {
                let k = i as f32 / segments as f32;
                
                let y = graph_area.bottom - graph_area.height() * k;

                paint.set_alpha_f(0.4);

                canvas.draw_line(
                    (graph_area.left, y),
                    (graph_area.right, y),
                    &paint
                );

                let label = formatter(k * y_len + self.range.1.start);
                let textblob = TextBlob::from_str(label.as_str(), &font).wrap_err("")?;

                let (_, bounds) = font.measure_str(label.as_str(), Some(&paint));
                let offset = bounds.height() / 2.;

                paint.set_alpha_f(1.);

                // canvas.save();
                // canvas.rotate(90., None);
                // canvas.draw_text_blob(textblob, (graph_area.left - 15.0, y + offset), &paint);
                canvas.draw_text_blob(textblob, (graph_area.left - 10.0, y + offset), &paint);
                // canvas.restore();
            }
        }

        paint
            .set_blend_mode(BlendMode::Lighten)
            .set_stroke_width(self.line_width);

        for graph in self.graphs.iter() {
            paint
                .set_color(graph.color)
                .set_shader(None)
                .set_style(PaintStyle::Stroke);

            let mut path = Path::new();
            
            for point in graph.points.iter()
                .sorted_by(|a, b| {
                    match a.0.partial_cmp(&b.0) {
                        Some(o) => o,
                        _ => Ordering::Equal
                    }
                })
            {
                let point = (
                    graph_area.left + (point.0 - self.range.0.start) / x_len * graph_area.width(),
                    graph_area.bottom - (point.1 - self.range.1.start) / y_len * graph_area.height()
                );

                if path.count_points() == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }

            canvas.draw_path(&path, &paint);

            path.line_to((graph_area.right, graph_area.bottom));
            path.line_to((graph_area.left, graph_area.bottom));

            match graph.fill {
                GraphFill::Solid(color) => {
                    paint.set_color(color).set_style(PaintStyle::Fill);
                    canvas.draw_path(&path, &paint);
                },
                GraphFill::Gradient(top, bottom) => {
                    let gradient = gradient_shader::linear(
                        ((0., graph_area.top), (0., graph_area.bottom)),
                        [top, bottom].as_slice(),
                        None,
                        TileMode::Clamp,
                        None,
                        None
                    ).wrap_err("")?;

                    paint.set_shader(Some(gradient)).set_style(PaintStyle::Fill);
                    canvas.draw_path(&path, &paint);
                }
                _ => {}
            }
        }

        paint
            .set_shader(None)
            .set_color(Color::WHITE)
            .set_alpha_f(1.0)
            .set_style(PaintStyle::Stroke)
            .set_blend_mode(BlendMode::default())
            .set_stroke_width(1.5);

        // TODO: Figure out how and when these should be drawn
        if self.draw_x_axis {
            canvas.draw_line((graph_area.left, graph_area.bottom), (graph_area.right, graph_area.bottom), &paint);
        }
        if self.draw_y_axis {
            canvas.draw_line((graph_area.left, graph_area.bottom), (graph_area.left, graph_area.top), &paint);
        }

        Ok((self.size.1 as i32, Some(surface)))
    }

    pub fn draw(&mut self) -> Result<Graph> {
        let surface = {
            let (legend_height, legend_surface) = match self.draw_legend {
                true => self.draw_legend().wrap_err("")?,
                false => (0, None)
            };
            let (graph_height, graph_surface) = self.draw_graph().wrap_err("")?;

            let sampling_options = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);

            let size = Rect::new(0., 0., self.size.0 as f32, (legend_height + graph_height) as f32);

            let mut surface = Surface::new_raster_n32_premul((size.width() as i32, size.height() as i32)).wrap_err("")?;
            let canvas = surface.canvas();
            let mut paint = Paint::default();
            paint.set_anti_alias(true);

            match self.background {
                GraphBackground::Color { color } => {
                    paint.set_color(color);
                    canvas.draw_rect(size, &paint);
                },
                GraphBackground::Image { image, dim } => {
                    let info = ImageInfo::new(
                        (image.width() as i32, image.height() as i32),
                        ColorType::RGBA8888,
                        AlphaType::Opaque,
                        None
                    );

                    let outset = if (size.width() / size.height()) / (image.width() as f32 / image.height() as f32) <= 1. {
                        (
                            (image.width() as f32 * size.height() / image.height() as f32 - size.width()) / 2.,
                            0.0
                        )
                    } else {
                        (
                            0.0, 
                            (image.height() as f32 * size.width() / image.width() as f32 - size.height()) / 2.
                        )
                    };

                    let bytes = image.to_rgba8();
                    let pixels = Data::new_copy(&bytes);
                    let row_bytes = image.width() * 4;

                    let img = Image::from_raster_data(&info, pixels, row_bytes as usize).wrap_err("")?;
                    canvas.draw_image_rect(img, None, size.with_outset(outset), &paint);

                    let dim = dim.clamp(0., 1.);

                    if dim > 0. {
                        paint.set_color(Color::BLACK).set_alpha_f(dim);
                        canvas.draw_rect(size, &paint);
                    }
                },
                _ => {}
            }

            if let Some(mut surface) = legend_surface {
                surface.draw(canvas, (0, 0), sampling_options, None);
            }
            if let Some(mut surface) = graph_surface {
                surface.draw(canvas, (0, legend_height), sampling_options, None);
            }

            surface
        };

        Ok(Graph::new(surface))
    }
}
