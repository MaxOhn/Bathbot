use super::ReactionWrapper;
use crate::{
    bail,
    bg_game::MapsetTags,
    core::Emote,
    database::MapsetTagWrapper,
    embeds::{BGStartEmbed, BGTagsEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, send_reaction, MessageExt},
    Args, BotResult, Context,
};

use rosu_v2::model::GameMode;
use std::{sync::Arc, time::Duration};
use tokio_stream::StreamExt;
use twilight_model::{
    channel::{Message, ReactionType},
    gateway::event::Event,
};

#[command]
#[bucket("bg_start")]
#[short_desc("Start the bg game or skip the current background")]
#[aliases("s", "resolve", "r", "skip")]
pub async fn start(ctx: Arc<Context>, msg: &Message, mut args: Args) -> BotResult<()> {
    match ctx.restart_game(msg.channel_id).await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    }

    let mode = match args.next() {
        Some("m") | Some("mania") => GameMode::MNA,
        _ => GameMode::STD,
    };

    let mapsets = match get_mapsets(&ctx, msg, mode).await {
        Ok(mapsets) => mapsets,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    if !(mapsets.is_empty() || ctx.has_running_game(msg.channel_id)) {
        let _ = ctx.http.create_typing_trigger(msg.channel_id).await;
        ctx.add_game_and_start(Arc::clone(&ctx), msg.channel_id, mapsets);
    }

    Ok(())
}

async fn get_mapsets(
    ctx: &Context,
    msg: &Message,
    mode: GameMode,
) -> BotResult<Vec<MapsetTagWrapper>> {
    if mode == GameMode::MNA {
        return ctx.psql().get_all_tags_mapset(GameMode::MNA).await;
    }

    // Send initial message
    let embed = BGStartEmbed::new(msg.author.id).into_builder().build();
    let response = msg.respond_embed(&ctx, embed).await?;

    // Prepare the reaction stream
    let self_id = match ctx.cache.current_user() {
        Some(user) => user.id,
        None => bail!("No CurrentUser in cache"),
    };

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
                "✅" if reaction.as_deref().user_id == msg.author.id => break,
                "❌" if reaction.as_deref().user_id == msg.author.id => {
                    msg.reply(ctx, "Game cancelled").await?;

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
            let _ = msg.error(ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
    };

    let data = BGTagsEmbed::new(included, excluded, mapsets.len());
    msg.respond_embed(&ctx, data.into_builder().build()).await?;

    if !mapsets.is_empty() {
        info!(
            "Starting bg game with included: {} - excluded: {}",
            included.join(','),
            excluded.join(',')
        );
    }

    Ok(mapsets)
}
