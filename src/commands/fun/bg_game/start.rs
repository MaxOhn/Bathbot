use super::ReactionWrapper;
use crate::{
    bail,
    bg_game::MapsetTags,
    database::MapsetTagWrapper,
    embeds::{BGStartEmbed, BGTagsEmbed, EmbedData},
    util::{constants::GENERAL_ISSUE, MessageExt},
    Args, BotResult, Context,
};

use rosu::model::GameMode;
use std::sync::Arc;
use tokio::{stream::StreamExt, time::Duration};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::{
    channel::{Message, ReactionType},
    gateway::{event::Event, payload::ReactionAdd},
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
    let data = BGStartEmbed::new(msg.author.id);
    let embed = data.build_owned().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Prepare the reaction stream
    let self_id = match ctx.cache.current_user() {
        Some(user) => user.id,
        None => bail!("No CurrentUser in cache"),
    };
    let reaction_add_stream = ctx
        .standby
        .wait_for_reaction_stream(response.id, move |event: &ReactionAdd| {
            event.user_id != self_id
        })
        .filter_map(|reaction: ReactionAdd| Some(ReactionWrapper::Add(reaction.0)));
    let reaction_remove_stream = ctx
        .standby
        .wait_for_event_stream(|_: &Event| true)
        .filter_map(|event: Event| {
            if let Event::ReactionRemove(reaction) = event {
                if reaction.0.message_id == response.id && reaction.0.user_id != self_id {
                    return Some(ReactionWrapper::Remove(reaction.0));
                }
            }
            None
        });
    let mut reaction_stream = reaction_add_stream
        .merge(reaction_remove_stream)
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
        let emote = RequestReactionType::Unicode {
            name: reaction.to_string(),
        };
        ctx.http
            .create_reaction(response.channel_id, response.id, emote)
            .await?;
    }
    let mut included = MapsetTags::empty();
    let mut excluded = MapsetTags::empty();

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
    let mapsets = match ctx
        .psql()
        .get_specific_tags_mapset(mode, included, excluded)
        .await
    {
        Ok(mapsets) => mapsets,
        Err(why) => {
            let _ = msg.error(ctx, GENERAL_ISSUE).await;
            return Err(why);
        }
    };
    let data = BGTagsEmbed::new(included, excluded, mapsets.len());
    let embed = data.build_owned().build()?;
    ctx.http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;
    if !mapsets.is_empty() {
        info!(
            "Starting bg game with included: {} - excluded: {}",
            included.join(','),
            excluded.join(',')
        );
    }
    Ok(mapsets)
}
