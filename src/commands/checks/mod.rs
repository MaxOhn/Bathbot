use crate::Guilds;
use serenity::{
    framework::standard::{macros::check, Args, CheckResult, CommandOptions},
    model::prelude::Message,
    prelude::Context,
};

#[check]
#[name = "Authority"]
#[check_in_help(true)]
#[display_in_help(true)]
fn authority_check(
    ctx: &mut Context,
    msg: &Message,
    _: &mut Args,
    _: &CommandOptions,
) -> CheckResult {
    if let Some(member) = msg.member(&ctx.cache) {
        if let Ok(permissions) = member.permissions(&ctx.cache) {
            // Does it have admin permission
            if permissions.administrator() {
                return CheckResult::Success;
            } else {
                // Does it have authority role
                for role_id in member.roles {
                    if let Some(role) = role_id.to_role_cached(&ctx.cache) {
                        let role_name = role.name.to_lowercase();
                        let data = ctx.data.read();
                        let guilds = data.get::<Guilds>().expect("Could not get Guilds");
                        let guild_id = msg.guild_id.unwrap();
                        let contains_authority = guilds
                            .get(&guild_id)
                            .unwrap_or_else(|| panic!("GuildId {} not found in Guilds", guild_id.0))
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
