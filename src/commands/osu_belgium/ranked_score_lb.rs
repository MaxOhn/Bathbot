use crate::{commands::checks::*, util::numbers};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};

#[command]
#[checks(MainGuild)]
#[description = "Show the ranked score leaderboard among all linked members in this server.\n\
                If no mode is specified it defaults to osu!standard."]
#[usage = "[mania / taiko / ctb]"]
async fn rankedscore(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mode = super::get_mode(args);
    let (users, next_update) = super::member_users(ctx, msg.guild_id.unwrap(), mode).await?;

    // Map to ranked scores, sort, then format
    let mut users: Vec<_> = users
        .into_iter()
        .map(|u| (u.username, u.ranked_score))
        .collect();
    users.sort_by(|(_, a), (_, b)| b.cmp(&a));
    let users: Vec<_> = users
        .into_iter()
        .map(|(name, score)| (name, numbers::with_comma_u64(score)))
        .collect();

    // Send response
    super::send_response(ctx, users, next_update, msg).await
}
