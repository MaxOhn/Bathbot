use std::sync::Arc;

use eyre::Report;
use hashbrown::hash_map::Entry;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    application::interaction::MessageComponentInteraction,
    channel::embed::{Embed, EmbedField},
};

use crate::{
    core::Context,
    embeds::{BGTagsEmbed, EmbedData},
    error::InvalidGameState,
    games::bg::GameWrapper,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{GENERAL_ISSUE, RED},
        Authored, ComponentExt,
    },
    BotResult,
};

use super::{Effects, GameState, MapsetTags};

pub async fn handle_bg_start_include(
    ctx: &Context,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    if let Some(GameState::Setup {
        author, included, ..
    }) = ctx.bg_games().write(component.channel_id).await.get_mut()
    {
        if *author != component.user_id()? {
            return Ok(());
        }

        *included = parse_component_tags(&component);
        update_field(ctx, &mut component, *included, "Included tags").await?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        let report = Report::new(err).wrap_err("failed to remove components");
        warn!("{report:?}");
    }

    Ok(())
}

pub async fn handle_bg_start_exclude(
    ctx: &Context,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    if let Some(GameState::Setup {
        author, excluded, ..
    }) = ctx.bg_games().write(component.channel_id).await.get_mut()
    {
        if *author != component.user_id()? {
            return Ok(());
        }

        *excluded = parse_component_tags(&component);
        update_field(ctx, &mut component, *excluded, "Excluded tags").await?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        let report = Report::new(err).wrap_err("failed to remove components");
        warn!("{report:?}");
    }

    Ok(())
}

pub async fn handle_bg_start_button(
    ctx: Arc<Context>,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    let channel = component.channel_id;

    match ctx.bg_games().write(channel).await.entry() {
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

                let mapset_fut =
                    ctx.psql()
                        .get_specific_tags_mapset(GameMode::Osu, *included, *excluded);

                let mapsets = match mapset_fut.await {
                    Ok(mapsets) => mapsets,
                    Err(err) => {
                        let embed = EmbedBuilder::new()
                            .color(RED)
                            .description(GENERAL_ISSUE)
                            .build();

                        if let Err(err) = remove_components(&ctx, &component, Some(embed)).await {
                            let report = Report::new(err).wrap_err("failed to remove components");
                            warn!("{report:?}");
                        }

                        return Err(err);
                    }
                };

                let embed =
                    BGTagsEmbed::new(*included, *excluded, mapsets.len(), *effects, *difficulty)
                        .build();

                if let Err(err) = remove_components(&ctx, &component, Some(embed)).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }

                if mapsets.is_empty() {
                    entry.remove();

                    return Ok(());
                }

                info!(
                    "Starting game with included: {} - excluded: {}",
                    included.join(','),
                    excluded.join(',')
                );

                let ctx = Arc::clone(&ctx);
                let game = GameWrapper::new(ctx, channel, mapsets, *effects, *difficulty).await;

                entry.insert(GameState::Running { game });
            }
            GameState::Running { .. } => {
                if let Err(err) = remove_components(&ctx, &component, None).await {
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(&ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_cancel(
    ctx: &Context,
    component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    match ctx.bg_games().write(component.channel_id).await.entry() {
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
                    let report = Report::new(err).wrap_err("failed to remove components");
                    warn!("{report:?}");
                }

                return Ok(());
            }
        },
        Entry::Vacant(_) => {
            if let Err(err) = remove_components(ctx, &component, None).await {
                let report = Report::new(err).wrap_err("failed to remove components");
                warn!("{report:?}");
            }
        }
    }

    Ok(())
}

pub async fn handle_bg_start_effects(
    ctx: &Context,
    mut component: Box<MessageComponentInteraction>,
) -> BotResult<()> {
    if let Some(GameState::Setup {
        author, effects, ..
    }) = ctx.bg_games().write(component.channel_id).await.get_mut()
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
                            warn!("unknown effects `{value}`");

                            return effects;
                        }
                    }
            });

        let mut embed = component
            .message
            .embeds
            .pop()
            .ok_or(InvalidGameState::MissingEmbed)?;

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
        component.callback(ctx, builder).await?;
    } else if let Err(err) = remove_components(ctx, &component, None).await {
        let report = Report::new(err).wrap_err("failed to remove components");
        warn!("{report:?}");
    }

    Ok(())
}

async fn update_field(
    ctx: &Context,
    component: &mut MessageComponentInteraction,
    tags: MapsetTags,
    name: &str,
) -> BotResult<()> {
    let mut embed = component
        .message
        .embeds
        .pop()
        .ok_or(InvalidGameState::MissingEmbed)?;

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
    component.callback(ctx, builder).await?;

    Ok(())
}

async fn remove_components(
    ctx: &Context,
    component: &MessageComponentInteraction,
    embed: Option<Embed>,
) -> BotResult<()> {
    let mut builder = MessageBuilder::new().components(Vec::new());

    if let Some(embed) = embed {
        builder = builder.embed(embed);
    }

    component.callback(ctx, builder).await?;

    Ok(())
}

fn parse_component_tags(component: &MessageComponentInteraction) -> MapsetTags {
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
                    warn!("unknown mapset tag `{value}`");

                    return tags;
                }
            }
        })
}
