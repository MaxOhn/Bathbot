use crate::{
    util::globals::{AVATAR_URL, MSG_MEMORY},
    Error, ResponseOwner,
};

use image::{
    imageops::FilterType, png::PNGEncoder, ColorType, DynamicImage, GenericImage, GenericImageView,
};
use regex::Regex;
use reqwest::Client;
use serenity::{
    cache::CacheRwLock,
    model::{
        channel::{EmbedField, Message},
        guild::Member,
        id::{ChannelId, MessageId, UserId},
    },
    prelude::{Context, RwLock, ShareMap},
};
use std::{str::FromStr, sync::Arc};
use tokio::runtime::Runtime;

pub fn get_combined_thumbnail(user_ids: &[u32]) -> Result<Vec<u8>, Error> {
    let mut combined = DynamicImage::new_rgba8(128, 128);
    let amount = user_ids.len() as u32;
    let w = 128 / amount;
    let client = Client::new();
    let mut rt = Runtime::new().unwrap();
    for (i, id) in user_ids.iter().enumerate() {
        let url = format!("{}{}", AVATAR_URL, id);
        let res = rt.block_on(async { client.get(&url).send().await?.bytes().await })?;
        let img =
            image::load_from_memory(res.as_ref())?.resize_exact(128, 128, FilterType::Lanczos3);
        let x = i as u32 * 128 / amount;
        for i in 0..w {
            for j in 0..128 {
                let pixel = img.get_pixel(x + i, j);
                combined.put_pixel(x + i, j, pixel);
            }
        }
    }
    let mut png_bytes: Vec<u8> = Vec::with_capacity(16_384); // 2^14 = 128x128
    let png_encoder = PNGEncoder::new(&mut png_bytes);
    png_encoder.encode(&combined.to_bytes(), 128, 128, ColorType::Rgba8)?;
    Ok(png_bytes)
}

pub fn map_id_from_history(msgs: Vec<Message>, cache: CacheRwLock) -> Option<u32> {
    let url_regex = Regex::new(r".*/([0-9]{1,9})").unwrap();
    let field_regex = Regex::new(r".*\{(\d+/){2,}\d+}.*").unwrap();
    let check_field = |url: &str, field: &EmbedField| {
        if field_regex.is_match(&field.value) {
            let result = url_regex
                .captures(url)
                .and_then(|caps| caps.get(1))
                .and_then(|cap| u32::from_str(cap.as_str()).ok());
            if result.is_some() {
                return result;
            }
        }
        None
    };
    for msg in msgs {
        if !msg.is_own(&cache) {
            continue;
        }
        for embed in msg.embeds {
            if let Some(author) = embed.author {
                if let Some(url) = author.url {
                    if url.contains("/b/") {
                        let result = url_regex
                            .captures(&url)
                            .and_then(|caps| caps.get(1))
                            .and_then(|cap| u32::from_str(cap.as_str()).ok());
                        if result.is_some() {
                            return result;
                        }
                    }
                }
            }
            if embed.fields.is_empty() {
                continue;
            }
            if let Some(url) = embed.url {
                if let Some(field) = embed.fields.first() {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
                if let Some(field) = embed.fields.get(5) {
                    if let Some(id) = check_field(&url, &field) {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

pub fn save_response_owner(msg_id: MessageId, author_id: UserId, data: Arc<RwLock<ShareMap>>) {
    let mut data = data.write();
    let (queue, owners) = data
        .get_mut::<ResponseOwner>()
        .expect("Could not get ResponseOwner");
    queue.push_front(msg_id);
    if queue.len() > MSG_MEMORY {
        let oldest = queue.pop_back().unwrap();
        owners.remove(&oldest);
    }
    owners.insert(msg_id, author_id);
}

pub fn get_member(ctx: &Context, channel_id: ChannelId, user_id: UserId) -> Option<Member> {
    channel_id
        .to_channel(ctx)
        .ok()
        .and_then(|channel| channel.guild())
        .and_then(|guild_channel| guild_channel.read().guild_id.member(ctx, user_id).ok())
}
