use crate::{util::globals::AVATAR_URL, Error};

use image::{
    imageops::FilterType, png::PNGEncoder, ColorType, DynamicImage, GenericImage, GenericImageView,
};
use regex::Regex;
use reqwest::Client;
use serenity::{
    async_trait,
    cache::Cache,
    model::{
        channel::{EmbedField, Message, ReactionType},
        guild::Member,
        id::{ChannelId, UserId},
    },
    prelude::{Context, RwLock, TypeMap},
};
use std::{str::FromStr, sync::Arc, time::Duration};
use tokio::stream::StreamExt;

pub async fn get_combined_thumbnail(user_ids: &[u32]) -> Result<Vec<u8>, Error> {
    let mut combined = DynamicImage::new_rgba8(128, 128);
    let amount = user_ids.len() as u32;
    let w = 128 / amount;
    let client = Client::new();
    for (i, id) in user_ids.iter().enumerate() {
        let url = format!("{}{}", AVATAR_URL, id);
        let res = client.get(&url).send().await?.bytes().await?;
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

pub async fn map_id_from_history(msgs: Vec<Message>, cache: &Cache) -> Option<u32> {
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
        if !msg.is_own(cache).await {
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

pub async fn get_member(ctx: &Context, channel_id: ChannelId, user_id: UserId) -> Option<Member> {
    match channel_id
        .to_channel(ctx)
        .await
        .ok()
        .and_then(|channel| channel.guild())
    {
        Some(guild_channel) => guild_channel.guild_id.member(ctx, user_id).await.ok(),
        None => None,
    }
}

pub trait CacheData {
    fn cache(&self) -> &Cache;
    fn data(&self) -> &Arc<RwLock<TypeMap>>;
}

impl CacheData for &Context {
    fn cache(&self) -> &Cache {
        &self.cache
    }
    fn data(&self) -> &Arc<RwLock<TypeMap>> {
        &self.data
    }
}

impl CacheData for (&mut Arc<Cache>, &mut Arc<RwLock<TypeMap>>) {
    fn cache(&self) -> &Cache {
        self.0
    }
    fn data(&self) -> &Arc<RwLock<TypeMap>> {
        self.1
    }
}

#[async_trait]
pub trait MessageExt: Sized {
    async fn reaction_delete(self, ctx: &Context, author: UserId);
}

#[async_trait]
impl MessageExt for Message {
    async fn reaction_delete(self, ctx: &Context, author: UserId) {
        let mut collector = self
            .await_reactions(ctx)
            .timeout(Duration::from_secs(60))
            .author_id(author)
            .filter(|reaction| {
                if let ReactionType::Unicode(reaction_name) = &reaction.emoji {
                    reaction_name == "❌"
                } else {
                    false
                }
            })
            .await;
        let http = Arc::clone(&ctx.http);
        tokio::spawn(async move {
            let delete_reaction = collector.next().await;
            if delete_reaction.is_some() {
                if let Err(why) = self.delete(&http).await {
                    warn!("Error while deleting msg after reaction: {}", why);
                }
            }
        });
    }
}
