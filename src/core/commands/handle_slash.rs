use crate::{
    commands::{fun, osu, owner, songs, tracking, twitch, utility},
    util::ApplicationCommandExt,
    BotResult, Context, Error,
};

use std::sync::Arc;
use twilight_model::application::interaction::ApplicationCommand;

pub async fn handle_interaction(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    // TODO: Ratelimit
    // TODO: Command count metric
    // TODO: Extend 3s response time for long commands

    let cmd_name = command.data.name.to_owned();
    log_slash(&ctx, &command, cmd_name.as_str());

    let command_result = match cmd_name.as_str() {
        "avatar" => osu::slash_avatar(ctx, command).await,
        "backgroundgame" => fun::slash_backgroundgame(ctx, command).await,
        "cache" => owner::slash_cache(ctx, command).await,
        "compare" => osu::slash_compare(ctx, command).await,
        "link" => osu::slash_link(ctx, command).await,
        "matchcost" => osu::slash_matchcost(ctx, command).await,
        "matchlive" => osu::slash_matchlive(ctx, command).await,
        "medal" => osu::slash_medal(ctx, command).await,
        "minesweeper" => fun::slash_minesweeper(ctx, command).await,
        "ping" => utility::slash_ping(ctx, command).await,
        "rank" => osu::slash_rank(ctx, command).await,
        "ranking" => osu::slash_ranking(ctx, command).await,
        "ratio" => osu::slash_ratio(ctx, command).await,
        "recent" => osu::slash_recent(ctx, command).await,
        "roll" => utility::slash_roll(ctx, command).await,
        "search" => osu::slash_mapsearch(ctx, command).await,
        "song" => songs::slash_song(ctx, command).await,
        "track" => tracking::slash_track(ctx, command).await,
        "trackstream" => twitch::slash_trackstream(ctx, command).await,
        _ => return Err(Error::UnknownSlashCommand(cmd_name)),
    };

    match command_result {
        Ok(_) => info!("Processed slash command `{}`", cmd_name),
        Err(why) => return Err(Error::Command(Box::new(why), cmd_name)),
    }

    Ok(())
}

fn log_slash(ctx: &Context, command: &ApplicationCommand, cmd_name: &str) {
    let username = command
        .username()
        .or_else(|| {
            command
                .member
                .as_ref()
                .and_then(|member| member.nick.as_deref())
        })
        .unwrap_or("<unknown user>");

    let mut location = String::with_capacity(31);

    match command.guild_id.and_then(|id| ctx.cache.guild(id)) {
        Some(guild) => {
            location.push_str(guild.name.as_str());
            location.push(':');

            match ctx.cache.guild_channel(command.channel_id) {
                Some(channel) => location.push_str(channel.name()),
                None => location.push_str("<uncached channel>"),
            }
        }
        None => location.push_str("Private"),
    }

    info!("[{}] {}: /{}", location, username, cmd_name);
}
