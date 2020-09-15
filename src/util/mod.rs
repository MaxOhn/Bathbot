pub mod constants;
mod country;
pub mod datetime;
pub mod exts;
pub mod matcher;
pub mod matrix;
pub mod numbers;
pub mod osu;
mod safe_content;

#[macro_use]
pub mod error;

use constants::DISCORD_CDN;
pub use country::{Country, SNIPE_COUNTRIES};
pub use exts::*;
pub use matrix::Matrix;
pub use safe_content::content_safe;

use crate::{BotResult, Context};

use futures::future::try_join_all;
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat::Png,
};
use twilight_model::id::UserId;

pub fn discord_avatar(user_id: UserId, hash: &str) -> String {
    format!("{}avatars/{}/{}.webp?size=1024", DISCORD_CDN, user_id, hash)
}

pub fn levenshtein_distance(word_a: &str, word_b: &str) -> usize {
    let (word_a, word_b) = if word_a.chars().count() > word_b.chars().count() {
        (word_b, word_a)
    } else {
        (word_a, word_b)
    };
    let mut costs: Vec<usize> = (0..=word_b.len()).collect();
    for (i, a) in (1..=word_a.len()).zip(word_a.chars()) {
        let mut last_val = i;
        for (j, b) in (1..=word_b.len()).zip(word_b.chars()) {
            let new_val = if a == b {
                costs[j - 1]
            } else {
                costs[j - 1].min(last_val).min(costs[j]) + 1
            };
            costs[j - 1] = last_val;
            last_val = new_val;
        }
        costs[word_b.len()] = last_val;
    }
    costs[word_b.len()]
}

pub async fn get_combined_thumbnail(ctx: &Context, user_ids: &[u32]) -> BotResult<Vec<u8>> {
    let mut combined = DynamicImage::new_rgba8(128, 128);
    let amount = user_ids.len() as u32;
    let w = 128 / amount;
    let pfp_futs = user_ids.iter().map(|id| ctx.clients.custom.get_avatar(*id));
    let pfps = try_join_all(pfp_futs).await?;
    for (i, pfp) in pfps.iter().enumerate() {
        let img = image::load_from_memory(pfp)?.resize_exact(128, 128, FilterType::Lanczos3);
        let x = i as u32 * 128 / amount;
        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(x + i, j);
                combined.put_pixel(x + i, j, pixel);
            }
        }
    }
    let mut png_bytes: Vec<u8> = Vec::with_capacity(16_384); // 2^14 = 128x128
    combined.write_to(&mut png_bytes, Png)?;
    Ok(png_bytes)
}
