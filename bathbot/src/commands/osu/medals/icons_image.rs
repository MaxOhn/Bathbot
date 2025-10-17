use std::iter;

use eyre::{ContextCompat, Result, WrapErr};
use image::imageops::FilterType::Lanczos3;
use plotters::{chart::ChartBuilder, prelude::IntoDrawingArea};
use plotters_skia::SkiaBackend;
use skia_safe::{EncodedImageFormat, surfaces};

use crate::commands::osu::BitMapElement;

pub fn draw_icons_image(icons: &[(u32, Vec<u8>)]) -> Result<Vec<u8>> {
    const W: u32 = 1417;
    const H: u32 = 376;
    const MARGIN: u32 = 3;

    if icons.is_empty() {
        return Ok(Vec::new());
    }

    let mut surface =
        surfaces::raster_n32_premul((W as i32, H as i32)).wrap_err("Failed to create surface")?;

    {
        let root = SkiaBackend::new(surface.canvas(), W, H).into_drawing_area();

        let tile_size = optimal_tile_size(W, H, icons.len() as u32);
        let per_row = W / tile_size;
        let icon_size = tile_size - MARGIN;

        let mut chart = ChartBuilder::on(&root)
            .build_cartesian_2d(0..W, 0..H)
            .wrap_err("Failed to build chart")?;

        chart
            .configure_mesh()
            .disable_mesh()
            .disable_axes()
            .draw()
            .wrap_err("Failed to draw icon mesh")?;

        for (chunk, row) in icons.chunks(per_row as usize).zip(0..) {
            let y = H - row * tile_size;

            for ((_, icon), col) in chunk.iter().zip(0..) {
                let icon_img = image::load_from_memory(icon)
                    .wrap_err("Failed to get icon from memory")?
                    .resize_exact(icon_size, icon_size, Lanczos3);

                let x = col * tile_size;

                let elem = BitMapElement::new(icon_img, (x, y));

                chart
                    .draw_series(iter::once(elem))
                    .wrap_err("Failed to draw icon")?;
            }
        }
    }

    let png_bytes = surface
        .image_snapshot()
        .encode(None, EncodedImageFormat::PNG, None)
        .wrap_err("Failed to encode image")?
        .to_vec();

    Ok(png_bytes)
}

/// Given a rectangle and an amount of square tiles, this function returns the
/// size of these tiles so that they can cover the rectangle optimally.
// credits to claude.ai :)
fn optimal_tile_size(width: u32, height: u32, num_tiles: u32) -> u32 {
    let mut best_tile_size = 0;

    // For very wide/tall rectangles, the optimal arrangement might be far from
    // square So we need to be smarter about our search range

    let sqrt_tiles = (num_tiles as f64).sqrt() as u32;

    // Check arrangements around square
    let square_range = sqrt_tiles.max(5);
    let start_square = 1.max(sqrt_tiles.saturating_sub(square_range));
    let end_square = width.min(num_tiles).min(sqrt_tiles + square_range);

    for tiles_per_row in start_square..=end_square {
        let tiles_per_col = num_tiles.div_ceil(tiles_per_row);

        if tiles_per_row <= width && tiles_per_col <= height {
            let max_tile_width = width / tiles_per_row;
            let max_tile_height = height / tiles_per_col;
            let tile_size = max_tile_width.min(max_tile_height);

            best_tile_size = best_tile_size.max(tile_size);
        }
    }

    // Also check extreme arrangements that might be optimal for very rectangular
    // spaces

    // Check arrangements with few rows (wide arrangements)
    let max_rows_to_check = height.min(20); // Don't check too many

    for tiles_per_col in 1..=max_rows_to_check {
        let tiles_per_row = num_tiles.div_ceil(tiles_per_col);

        if tiles_per_row <= width && tiles_per_col <= height {
            let max_tile_width = width / tiles_per_row;
            let max_tile_height = height / tiles_per_col;
            let tile_size = max_tile_width.min(max_tile_height);

            best_tile_size = best_tile_size.max(tile_size);
        }
    }

    // Check arrangements with few columns (tall arrangements)
    let max_cols_to_check = width.min(20);
    for tiles_per_row in 1..=max_cols_to_check {
        let tiles_per_col = num_tiles.div_ceil(tiles_per_row);

        if tiles_per_row <= width && tiles_per_col <= height {
            let max_tile_width = width / tiles_per_row;
            let max_tile_height = height / tiles_per_col;
            let tile_size = max_tile_width.min(max_tile_height);

            best_tile_size = best_tile_size.max(tile_size);
        }
    }

    best_tile_size
}
