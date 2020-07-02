use crate::{
    util::{discord::add_guild, globals::GENERAL_ISSUE, MessageExt},
    BgVerified, Guilds,
};

use serenity::{
    framework::standard::{macros::check, Args, CheckResult, CommandOptions},
    model::prelude::Message,
    prelude::Context,
};

#[check]
#[name = "Authority"]
#[check_in_help(true)]
#[display_in_help(true)]
async fn authority_check(
    ctx: &Context,
    msg: &Message,
    _: &mut Args,
    _: &CommandOptions,
) -> CheckResult {
    if let Some(member) = msg.member(&ctx.cache).await {
        if let Ok(permissions) = member.permissions(&ctx.cache).await {
            // Make sure guild is available
            let guild_id = msg.guild_id.unwrap();
            let contains_guild = {
                let data = ctx.data.read().await;
                let guilds = data.get::<Guilds>().unwrap();
                guilds.contains_key(&guild_id)
            };
            if !contains_guild {
                if let Err(why) = add_guild(ctx, guild_id).await {
                    warn!("Error while adding guild: {}", why);
                    let response = msg.channel_id.say(ctx, GENERAL_ISSUE).await;
                    if let Ok(response) = response {
                        response.reaction_delete(ctx, msg.author.id).await;
                    }
                    return false.into();
                }
            }
            // Does it have admin permission
            if permissions.administrator() {
                return CheckResult::Success;
            } else {
                // Does it have authority role
                for role_id in member.roles {
                    if let Some(role) = role_id.to_role_cached(&ctx.cache).await {
                        let role_name = role.name.to_lowercase();
                        let data = ctx.data.read().await;
                        let guilds = data.get::<Guilds>().unwrap();
                        let contains_authority = guilds
                            .get(&guild_id)
                            .unwrap()
                            .authorities
                            .contains(&role_name);
                        if contains_authority {
                            return CheckResult::Success;
                        }
                    }
                }
            }
        }
        false.into()
    // Is it in private channel
    } else {
        CheckResult::Success
    }
}

#[check]
#[name = "BgVerified"]
#[check_in_help(true)]
#[display_in_help(true)]
async fn bgverified_check(
    ctx: &Context,
    msg: &Message,
    _: &mut Args,
    _: &CommandOptions,
) -> CheckResult {
    let data = ctx.data.read().await;
    data.get::<BgVerified>()
        .unwrap()
        .contains(&msg.author.id)
        .into()
}
