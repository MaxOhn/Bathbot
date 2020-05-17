use crate::commands::checks::*;

use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::channel::Message,
    prelude::Context,
};

#[command]
#[checks(MainGuild)]
#[description = "Show the join date leaderboard among all linked members in this server.\n\
                If no mode is specified it defaults to osu!standard."]
#[usage = "[mania / taiko / ctb]"]
async fn joindate(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let mode = super::get_mode(args);
    let (users, next_update) = super::member_users(ctx, msg.guild_id.unwrap(), mode).await?;

    // Map to join date, sort, then format
    let mut users: Vec<_> = users
        .into_iter()
        .map(|u| (u.username, u.join_date))
        .collect();
    users.sort_by(|(_, a), (_, b)| a.cmp(&b));
    let users: Vec<_> = users
        .into_iter()
        .map(|(name, date)| (name, date.format("%F %T").to_string()))
        .collect();

    // Send response
    super::send_response(ctx, users, next_update, msg).await
}
