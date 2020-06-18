use crate::{
    commands::checks::*,
    database::MySQL,
    util::{globals::GENERAL_ISSUE, MessageExt},
    Guilds,
};

use itertools::Itertools;
use regex::Regex;
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::{id::RoleId, prelude::Message},
    prelude::Context,
};
use std::{collections::hash_map::Entry, str::FromStr};

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "Decide which roles should be considered authority roles. \
                 Authority roles enable the usage of certain commands like \
                 `<addstream` or `<roleassign`. Roles can be given as \
                 mention, as role name, or as role id (up to 10 roles possible).\n\
                 If you want to see the current authority roles, just pass \
                 `-show` as argument"]
#[usage = "[role1] [role2] ..."]
#[example = "-show"]
#[example = "mod \"bot commander\" moderator"]
#[aliases("authority", "setauthorities")]
async fn authorities(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();

    // Check if the user just wants to see the current authorities
    if args.current().unwrap_or_default() == "-show" {
        let data = ctx.data.read().await;
        let guilds = data.get::<Guilds>().unwrap();
        if let Some(guild) = guilds.get(&guild_id) {
            let roles = &guild.authorities;
            let content = if roles.is_empty() {
                "None".to_string()
            } else {
                roles.iter().map(|role| format!("`{}`", role)).join(", ")
            };

            // Send the message
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("Current authority roles for this server: {}", content),
                )
                .await?;
            return Ok(());
        } else {
            msg.channel_id.say(&ctx.http, GENERAL_ISSUE).await?;
            return Err(CommandError(format!(
                "GuildId {} not found in Guilds",
                guild_id.0
            )));
        }
    }
    // Get all roles of the guild
    let guild = guild_id.to_guild_cached(&ctx.cache).await.unwrap();
    let guild_roles = &guild.roles;
    // Parse the arguments
    let mut new_auth = Vec::with_capacity(10);
    let regex = Regex::new(r"<@&([0-9]*)>$").unwrap();
    while !args.is_empty() && new_auth.len() < 10 {
        let next = args.single_quoted::<String>()?.to_lowercase();
        // Check if role is given by name
        let entry = guild_roles
            .iter()
            .find(|(_, role)| role.name.to_lowercase() == next);
        if let Some((id, _)) = entry {
            new_auth.push(*id);
        } else {
            let role = u64::from_str(&next) // Given as id?
                .ok()
                .map(RoleId)
                .and_then(|id| {
                    // Valid role id?
                    if guild_roles.contains_key(&id) {
                        Some(id)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    // Given as mention?
                    regex
                        .captures(&next)
                        .and_then(|caps| caps.get(1))
                        .and_then(|id| u64::from_str(id.as_str()).ok())
                        .map(RoleId)
                        .and_then(|id| {
                            // Valid role id?
                            if guild_roles.contains_key(&id) {
                                Some(id)
                            } else {
                                None
                            }
                        })
                });
            if let Some(role) = role {
                new_auth.push(role);
            } else {
                msg.channel_id
                    .say(
                        &ctx.http,
                        format!("I don't know what role you mean with `{}`", next),
                    )
                    .await?;
                return Ok(());
            }
        }
    }
    let member = guild_id.member(ctx, msg.author.id).await?;
    let is_admin = member.permissions(&ctx.cache).await?.administrator();
    // If the new roles do not contain any of the members current roles
    let invalid_auths = !is_admin
        && member
            .roles(&ctx.cache)
            .await
            .unwrap()
            .iter()
            .find(|role| new_auth.contains(&role.id))
            == None;
    if invalid_auths {
        msg.channel_id
            .say(
                &ctx.http,
                "You cannot set authority roles to something \
             that would make you lose authority status.",
            )
            .await?;
        return Ok(());
    }
    let auth_strings: Vec<_> = new_auth
        .into_iter()
        .map(|id| {
            let mut name = guild_roles.get(&id).unwrap().name.clone();
            if name.contains(' ') {
                name.insert(0, '"');
                name.push('\"');
            }
            name
        })
        .collect();

    // Save in Guilds data
    {
        let mut data = ctx.data.write().await;
        let guilds = data.get_mut::<Guilds>().unwrap();
        match guilds.entry(guild_id) {
            Entry::Occupied(mut entry) => entry.get_mut().authorities = auth_strings.clone(),
            Entry::Vacant(_) => {
                msg.channel_id.say(&ctx.http, GENERAL_ISSUE).await?;
                return Err(CommandError(format!(
                    "GuildId {} not found in Guilds",
                    guild_id.0
                )));
            }
        }
    }

    // Save in database
    {
        let auth_string = auth_strings.iter().join(" ");
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        match mysql
            .update_guild_authorities(guild_id.0, auth_string)
            .await
        {
            Ok(_) => debug!("Updated authorities for guild id {}", guild_id.0),
            Err(why) => error!("Could not update authorities of guild: {}", why),
        }
    }

    // Send the message
    let content = if auth_strings.is_empty() {
        "None".to_string()
    } else {
        auth_strings
            .into_iter()
            .map(|role| format!("`{}`", role))
            .join(", ")
    };
    msg.channel_id
        .say(
            &ctx.http,
            format!("Successfully changed the authority roles to: {}", content),
        )
        .await?
        .reaction_delete(ctx, msg.author.id)
        .await;
    Ok(())
}
