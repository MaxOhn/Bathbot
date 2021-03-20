pub mod constants;
mod country;
pub mod datetime;
pub mod error;
pub mod exts;
pub mod matcher;
pub mod matrix;
pub mod numbers;
pub mod osu;
mod safe_content;

use constants::DISCORD_CDN;
pub use country::{Country, SNIPE_COUNTRIES};
pub use exts::*;
pub use matrix::Matrix;
pub use safe_content::content_safe;

use crate::{BotResult, Context};

use futures::stream::{FuturesOrdered, StreamExt};
use image::{
    imageops::FilterType, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat::Png,
};
use std::{collections::HashSet, iter::Extend};
use tokio::time::{sleep, Duration};
use twilight_model::id::{GuildId, UserId};
#[inline]
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

pub async fn get_combined_thumbnail(
    ctx: &Context,
    user_ids: impl Iterator<Item = u32>,
) -> BotResult<Vec<u8>> {
    let mut combined = DynamicImage::new_rgba8(128, 128);

    //  Careful here! Be sure the type implements size_hint accurately
    let amount = user_ids.size_hint().0 as u32;
    let w = 128 / amount;

    // Future stream
    let mut pfp_futs = user_ids
        .into_iter()
        .map(|id| ctx.clients.custom.get_avatar(id))
        .collect::<FuturesOrdered<_>>();

    let mut next = pfp_futs.next().await;
    let mut i = 0;

    // Closure that stitches the stripe onto the combined image
    let mut img_combining = |img: DynamicImage, i: u32| {
        let img = img.resize_exact(128, 128, FilterType::Lanczos3);
        let x = i as u32 * 128 / amount;

        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(x + i, j);
                combined.put_pixel(x + i, j, pixel);
            }
        }
    };

    // Process the stream elements
    while let Some(pfp_result) = next {
        let pfp = pfp_result?;
        let img = image::load_from_memory(&pfp)?;
        let (res, _) = tokio::join!(pfp_futs.next(), async { img_combining(img, i) });
        next = res;
        i += 1;
    }

    let mut png_bytes: Vec<u8> = Vec::with_capacity(16_384); // 2^14 = 128x128
    combined.write_to(&mut png_bytes, Png)?;

    Ok(png_bytes)
}

pub async fn get_member_ids(ctx: &Context, guild_id: GuildId) -> BotResult<HashSet<u64>> {
    let members = ctx
        .http
        .guild_members(guild_id)
        .limit(1000)
        .unwrap()
        .await?;

    let mut last = members.last().unwrap().user.id;
    let mut members: HashSet<_> = members.into_iter().map(|member| member.user.id.0).collect();

    if members.len() == 1000 {
        let delay = Duration::from_millis(500);

        while {
            sleep(delay).await;

            let new_members: Vec<_> = ctx
                .http
                .guild_members(guild_id)
                .limit(1000)
                .unwrap()
                .after(last)
                .await?;

            last = new_members.last().unwrap().user.id;
            let more_iterations = new_members.len() == 1000;
            members.extend(new_members.into_iter().map(|member| member.user.id.0));

            more_iterations
        } {}
    }

    Ok(members)
}
