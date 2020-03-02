use crate::{
    arguments::RoleArgs,
    commands::checks::*,
    database::MySQL,
    util::{discord, globals::GENERAL_ISSUE},
    Guilds,
};

use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};
use std::collections::hash_map::Entry;

#[command]
#[only_in("guild")]
#[checks(Authority)]
#[description = "Assign one of this server's roles to be the VC role. \
                 Whenever a member joins a VC channel other than the afk \
                 channel I will give them the VC role. When they leave VC \
                 or swap to the afk channel, I will remove the role again. \
                 If no role is given as argument, I will not consider any \
                 role to be this server's VC role."]
#[usage = "[role mention / role id]"]
#[example = "@In VC"]
fn vcrole(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let args = RoleArgs::new(args);
    let role = args.role_id;
    let guild_id = msg.guild_id.unwrap();
    let changed = {
        let mut data = ctx.data.write();
        let guilds = data.get_mut::<Guilds>().expect("Could not get Guilds");
        match guilds.entry(guild_id) {
            Entry::Occupied(mut entry) => {
                let mut value = entry.get_mut();
                if let Some(role) = role {
                    if let Some(prev_role) = &value.vc_role {
                        if prev_role != &role {
                            value.vc_role = Some(role);
                            true
                        } else {
                            false
                        }
                    } else {
                        value.vc_role = Some(role);
                        true
                    }
                } else if value.vc_role.is_some() {
                    value.vc_role = None;
                    true
                } else {
                    false
                }
            }
            Entry::Vacant(_) => {
                msg.channel_id.say(&ctx.http, GENERAL_ISSUE)?;
                return Err(CommandError(format!(
                    "GuildId {} not found in Guilds",
                    guild_id.0
                )));
            }
        }
    };

    if changed {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Err(why) = mysql.update_guild_vc_role(guild_id.0, role.map(|r| r.0)) {
            error!("Error while updating guild vc role: {}", why);
        }
    }

    let content = if let Some(role) = role {
        match role.to_role_cached(&ctx.cache) {
            Some(role) => format!("The role `{}` is now this server's VC role", role.name),
            None => "I couldn't find any role with that id. Maybe try giving the role as a mention"
                .to_string(),
        }
    } else {
        "I've set this server's VC role to `None`".to_string()
    };
    let response = msg.channel_id.say(&ctx.http, content)?;

    // Save the response owner
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}
