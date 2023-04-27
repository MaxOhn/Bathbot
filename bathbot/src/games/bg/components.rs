use std::sync::Arc;

use bathbot_psql::model::games::DbMapTagsParams;
use bathbot_util::{
    constants::{GENERAL_ISSUE, RED},
    EmbedBuilder, MessageBuilder,
};
use eyre::{ContextCompat, Result, WrapErr};
use hashbrown::hash_map::Entry;
use rosu_v2::prelude::GameMode;
use twilight_model::channel::message::embed::{Embed, EmbedField};

use super::{Effects, GameState, MapsetTags};
use crate::{
    core::Context,
    embeds::{BGTagsEmbed, EmbedData},
    games::bg::GameWrapper,
    util::{interaction::InteractionComponent, Authored, ComponentExt},
};

pub async fn handle_bg_start_include(
    ctx: &Context,
    mut component: InteractionComponent,
) -> Result<()> {
    let channel = component.channel_id;

    if let Some(GameState::Setup {
        author, included, ..
    }) = ctx.bg_games().write(&channel).await.get_mut()
    {
        if *author != component.user_id()? {
            return Ok(());
        }

        *included = parse_component_tags(&component);

        update_field(ctx, &mut component, *included, "Included tags")
            .await
            .wrap_err("Failed to update field")?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        warn!("{err:?}");
    }

    Ok(())
}

pub async fn handle_bg_start_exclude(
    ctx: &Context,
    mut component: InteractionComponent,
) -> Result<()> {
    let channel = component.channel_id;

    if let Some(GameState::Setup {
        author, excluded, ..
    }) = ctx.bg_games().write(&channel).await.get_mut()
    {
        if *author != component.user_id()? {
            return Ok(());
        }

        *excluded = parse_component_tags(&component);

        update_field(ctx, &mut component, *excluded, "Excluded tags")
            .await
            .wrap_err("Failed to update field")?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        warn!("{err:?}");
    }

    Ok(())
}

pub async fn handle_bg_start_button(
    ctx: Arc<Context>,
    component: InteractionComponent,
) -> Result<()> {
    let channel = component.channel_id;

    match ctx.bg_games().own(channel).await.entry() {
        Entry::Occupied(mut entry) => match entry.get() {
            GameState::Setup {
                author,
                difficulty,
                effects,
                excluded,
                included,
            } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                let mut params = DbMapTagsParams::new(GameMode::Osu);

                params.include(*included);
                params.exclude(*excluded);

                let entries = match ctx.games().bggame_tags(params).await {
                    Ok(entries) => entries,
                    Err(err) => {
                        let embed = EmbedBuilder::new()
                            .color(RED)
                            .description(GENERAL_ISSUE)
                            .build();

                        if let Err(err) = remove_components(&ctx, &component, Some(embed)).await {
                            warn!("{err:?}");
                        }

                        return Err(err);
                    }
                };

                let embed = BGTagsEmbed::new(
                    *included,
                    *excluded,
                    entries.tags.len(),
                    *effects,
                    *difficulty,
                );

                if let Err(err) = remove_components(&ctx, &component, Some(embed.build())).await {
                    warn!("{err:?}");
                }

                if entries.tags.is_empty() {
                    entry.remove();

                    return Ok(());
                }

                info!(
                    included = included.join(','),
                    excluded = excluded.join(','),
                    "Starting game"
                );

                let ctx = Arc::clone(&ctx);
                let game = GameWrapper::new(ctx, channel, entries, *effects, *difficulty).await;

                entry.insert(GameState::Running { game });
            }
            GameState::Running { .. } => {
                if let Err(err) = remove_components(&ctx, &component, None).await {
                    warn!("{err:?}");
                }
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(&ctx, &component, None).await {
                warn!("{err:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_cancel(ctx: &Context, component: InteractionComponent) -> Result<()> {
    match ctx.bg_games().own(component.channel_id).await.entry() {
        Entry::Occupied(entry) => match entry.get() {
            GameState::Setup { author, .. } => {
                if *author != component.user_id()? {
                    return Ok(());
                }

                let embed = EmbedBuilder::new()
                    .description("Aborted background game setup")
                    .build();

                entry.remove();
                remove_components(ctx, &component, Some(embed)).await?;
            }
            GameState::Running { .. } => {
                if let Err(err) = remove_components(ctx, &component, None).await {
                    warn!("{err:?}");
                }

                return Ok(());
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                warn!("{err:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_effects(
    ctx: &Context,
    mut component: InteractionComponent,
) -> Result<()> {
    if let Some(GameState::Setup {
        author, effects, ..
    }) = ctx.bg_games().write(&component.channel_id).await.get_mut()
    {
        if *author != component.user_id()? {
            return Ok(());
        }

        *effects = component
            .data
            .values
            .iter()
            .fold(Effects::empty(), |effects, value| {
                effects
                    | match value.as_str() {
                        "blur" => Effects::Blur,
                        "contrast" => Effects::Contrast,
                        "flip_h" => Effects::FlipHorizontal,
                        "flip_v" => Effects::FlipVertical,
                        "grayscale" => Effects::Grayscale,
                        "invert" => Effects::Invert,
                        _ => {
                            warn!(value, "Unknown effects");

                            return effects;
                        }
                    }
            });

        let mut embed = component.message.embeds.pop().wrap_err("missing embed")?;

        let field_opt = embed
            .fields
            .iter_mut()
            .find(|field| field.name == "Effects");

        if let Some(field) = field_opt {
            field.value = effects.join(", ");
        } else {
            let field = EmbedField {
                inline: false,
                name: "Effects".to_owned(),
                value: effects.join(", "),
            };

            embed.fields.push(field);
        }

        let builder = MessageBuilder::new().embed(embed);
        component
            .callback(ctx, builder)
            .await
            .wrap_err("failed to callback")?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        warn!("{err:?}");
    }

    Ok(())
}

async fn update_field(
    ctx: &Context,
    component: &mut InteractionComponent,
    tags: MapsetTags,
    name: &str,
) -> Result<()> {
    let mut embed = component.message.embeds.pop().wrap_err("missing embed")?;

    let field_opt = embed.fields.iter_mut().find(|field| field.name == name);

    if let Some(field) = field_opt {
        field.value = tags.join(", ");
    } else {
        let field = EmbedField {
            inline: false,
            name: name.to_owned(),
            value: tags.join(", "),
        };

        embed.fields.push(field);
    }

    let builder = MessageBuilder::new().embed(embed);
    component
        .callback(ctx, builder)
        .await
        .wrap_err("failed to callback")?;

    Ok(())
}

async fn remove_components(
    ctx: &Context,
    component: &InteractionComponent,
    embed: Option<Embed>,
) -> Result<()> {
    let mut builder = MessageBuilder::new().components(Vec::new());

    if let Some(embed) = embed {
        builder = builder.embed(embed);
    }

    component
        .callback(ctx, builder)
        .await
        .wrap_err("failed to callback to remove components")?;

    Ok(())
}

fn parse_component_tags(component: &InteractionComponent) -> MapsetTags {
    component
        .data
        .values
        .iter()
        .fold(MapsetTags::empty(), |tags, value| {
            tags | match value.as_str() {
                "easy" => MapsetTags::Easy,
                "hard" => MapsetTags::Hard,
                "meme" => MapsetTags::Meme,
                "weeb" => MapsetTags::Weeb,
                "kpop" => MapsetTags::Kpop,
                "farm" => MapsetTags::Farm,
                "hardname" => MapsetTags::HardName,
                "alt" => MapsetTags::Alternate,
                "bluesky" => MapsetTags::BlueSky,
                "english" => MapsetTags::English,
                "streams" => MapsetTags::Streams,
                "old" => MapsetTags::Old,
                "tech" => MapsetTags::Tech,
                _ => {
                    warn!(value, "Unknown mapset tag");

                    return tags;
                }
            }
        })
}
