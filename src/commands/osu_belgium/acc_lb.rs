use crate::{commands::checks::*, util::numbers};

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};
use std::cmp::Ordering;

#[command]
#[checks(MainGuild)]
#[description = "Show the accuracy leaderboard among all linked members in this server.\n\
                If no mode is specified it defaults to osu!standard."]
#[usage = "[mania / taiko / ctb]"]
async fn acc(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mode = super::get_mode(args);
    let (users, next_update) =
        super::member_users(ctx, msg.channel_id, msg.guild_id.unwrap(), mode).await?;

    // Map to accs, sort, then format
    let mut users: Vec<_> = users
        .into_iter()
        .map(|u| (u.username, u.accuracy))
        .collect();
    users.sort_by(|(_, a), (_, b)| b.partial_cmp(&a).unwrap_or_else(|| Ordering::Equal));
    let users: Vec<_> = users
        .into_iter()
        .map(|(name, acc)| (name, format!("{}%", numbers::round(acc))))
        .collect();

    // Send response
    super::send_response(ctx, users, next_update, msg).await
}
