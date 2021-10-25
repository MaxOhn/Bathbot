use super::ReactionWrapper;
use crate::{
    bg_game::MapsetTags,
    database::MapsetTagWrapper,
    embeds::{BGStartEmbed, BGTagsEmbed, EmbedData},
    util::{
        constants::{common_literals::MANIA, GENERAL_ISSUE},
        send_reaction, Emote, MessageExt,
    },
    BotResult, CommandData, Context, MessageBuilder,
};

use rosu_v2::model::GameMode;
use std::{sync::Arc, time::Duration};
use tokio_stream::StreamExt;
use twilight_model::{channel::ReactionType, gateway::event::Event};

pub(super) async fn restart(ctx: &Context, data: &CommandData<'_>) -> BotResult<bool> {
    match ctx.restart_game(data.channel_id()).await {
        Ok(restarted) => Ok(restarted),
        Err(why) => {
            let _ = data.error(ctx, GENERAL_ISSUE).await;

            Err(why)
        }
    }
}

#[command]
#[bucket("bg_start")]
#[short_desc("Start the bg game or skip the current background")]
#[aliases("s", "resolve", "r", "skip")]
async fn start(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let mode = match args.next() {
                Some("m") | Some(MANIA) => GameMode::MNA,
                _ => GameMode::STD,
            };

            _start(ctx, CommandData::Message { msg, args, num }, mode).await
        }
        CommandData::Interaction { .. } => unreachable!(),
    }
}

pub async fn _start(ctx: Arc<Context>, data: CommandData<'_>, mode: GameMode) -> BotResult<()> {
    if restart(&ctx, &data).await? {
        return Ok(());
    }

    let mapsets = match get_mapsets(&ctx, &data, mode).await {
        Ok(mapsets) => mapsets,
        Err(why) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let channel_id = data.channel_id();

    if !(mapsets.is_empty() || ctx.has_running_game(channel_id)) {
        Context::add_game_and_start(ctx, channel_id, mapsets);
    }

    Ok(())
}

async fn get_mapsets(
    ctx: &Context,
    data: &CommandData<'_>,
    mode: GameMode,
) -> BotResult<Vec<MapsetTagWrapper>> {
    if mode == GameMode::MNA {
        return ctx.psql().get_all_tags_mapset(GameMode::MNA).await;
    }

    let author_id = data.author()?.id;

    // Send initial message
    let builder = BGStartEmbed::new(author_id).into_builder().build().into();
    let response_raw = data.create_message(ctx, builder).await?;

    // Prepare the reaction stream
    let self_id = match ctx.cache.current_user() {
        Some(user) => user.id,
        None => bail!("No CurrentUser in cache"),
    };

    let response = response_raw.model().await?;
    let response_id = response.id;

    let reaction_stream = ctx
        .standby
        .wait_for_event_stream(move |event: &Event| match event {
            Event::ReactionAdd(event) => {
                event.message_id == response_id && event.user_id != self_id
            }
            Event::ReactionRemove(event) => {
                event.message_id == response_id && event.user_id != self_id
            }
            _ => false,
        })
        .map(|event| match event {
            Event::ReactionAdd(add) => ReactionWrapper::Add(add.0),
            Event::ReactionRemove(remove) => ReactionWrapper::Remove(remove.0),
            _ => unreachable!(),
        })
        .timeout(Duration::from_secs(60));

    // Send initial reactions
    let reactions = [
        "🍋",
        "🤓",
        "🤡",
        "🎨",
        "🍨",
        "👨‍🌾",
        "😱",
        "🪀",
        "🟦",
        "🗽",
        "🌀",
        "👴",
        "💯",
        "✅",
        "❌",
    ];

    for &reaction in reactions.iter() {
        let emote = Emote::Custom(reaction);
        send_reaction(&*ctx, &response, emote).await?;
    }

    let mut included = MapsetTags::empty();
    let mut excluded = MapsetTags::empty();

    tokio::pin!(reaction_stream);

    // Start collecting
    while let Some(Ok(reaction)) = reaction_stream.next().await {
        let tag = if let ReactionType::Unicode { ref name } = reaction.as_deref().emoji {
            match name.as_str() {
                "🍋" => MapsetTags::Easy,
                "🤓" => MapsetTags::Hard,
                "🤡" => MapsetTags::Meme,
                "👴" => MapsetTags::Old,
                "😱" => MapsetTags::HardName,
                "🟦" => MapsetTags::BlueSky,
                "🪀" => MapsetTags::Alternate,
                "🗽" => MapsetTags::English,
                "👨‍🌾" => MapsetTags::Farm,
                "💯" => MapsetTags::Tech,
                "🎨" => MapsetTags::Weeb,
                "🌀" => MapsetTags::Streams,
                "🍨" => MapsetTags::Kpop,
                "✅" if reaction.as_deref().user_id == author_id => break,
                "❌" if reaction.as_deref().user_id == author_id => {
                    let builder = MessageBuilder::new().content("Game cancelled");
                    response.create_message(ctx, builder).await?;

                    return Ok(Vec::new());
                }
                _ => continue,
            }
        } else {
            continue;
        };

        match reaction {
            ReactionWrapper::Add(_) => {
                included.insert(tag);
                excluded.remove(tag);
            }
            ReactionWrapper::Remove(_) => {
                excluded.insert(tag);
                included.remove(tag);
            }
        }
    }

    // Get all mapsets matching the given tags
    debug_assert_eq!(mode, GameMode::STD);

    let mapset_fut = ctx
        .psql()
        .get_specific_tags_mapset(mode, included, excluded);

    let mapsets = match mapset_fut.await {
        Ok(mapsets) => mapsets,
        Err(why) => {
            let _ = response.error(ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let data = BGTagsEmbed::new(included, excluded, mapsets.len());
    let builder = data.into_builder().build().into();

    response.create_message(ctx, builder).await?;

    if !mapsets.is_empty() {
        info!(
            "Starting bg game with included: {} - excluded: {}",
            included.join(','),
            excluded.join(',')
        );
    }

    Ok(mapsets)
}
