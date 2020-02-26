use crate::util::globals::AUTHORITY_ROLES;
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
                        if AUTHORITY_ROLES.contains(&role.name.to_lowercase().as_ref()) {
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
