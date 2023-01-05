use std::sync::Arc;

use bathbot_macros::SlashCommand;
use bathbot_util::{
    boyer_moore::contains_disallowed_infix, constants::OSU_API_ISSUE, MessageBuilder,
};
use eyre::{Report, Result};
use futures::{future, stream::FuturesUnordered, TryStreamExt};
use rosu_v2::prelude::{GameMode, OsuError};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::Context,
    embeds::ClaimNameEmbed,
    embeds::EmbedData,
    manager::redis::osu::{User, UserArgs},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "claimname",
    help = "If a player has not signed in for at least 6 months and has no plays,\
    their username may be claimed.\n\
    If that player does have any plays across all game modes, \
    a [non-linear function](https://www.desmos.com/calculator/b89siyv9j8) is used to calculate \
    how much extra time is added to those 6 months.\n\
    This is to prevent people from stealing the usernames of active or recently retired players."
)]
/// Check how much longer to wait until a name is up for grabs
pub struct ClaimName {
    /// Specify a username
    name: String,
}

async fn slash_claimname(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let ClaimName { name } = ClaimName::from_interaction(command.input_data())?;

    let content = if name.chars().count() > 15 {
        Some("Names can have at most 15 characters so your name won't be accepted".to_owned())
    } else if let Some(c) = name
        .chars()
        .find(|c| !matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '[' | ']' | '_' | ' '))
    {
        Some(format!(
            "`{c}` is an invalid character for usernames so `{name}` won't be accepted"
        ))
    } else if name.len() < 3 {
        Some(format!(
            "Names must be at least 3 characters long so `{name}` won't be accepted"
        ))
    } else if name.contains('_') && name.contains(' ') {
        Some(format!(
            "Names may contains underscores or spaces but not both \
            so `{name}` won't be accepted"
        ))
    } else if name.starts_with(' ') || name.ends_with(' ') {
        Some(format!(
            "Names can't start or end with spaces so `{name}` won't be accepted"
        ))
    } else {
        None
    };

    if let Some(content) = content {
        let builder = MessageBuilder::new().embed(content);
        command.update(&ctx, &builder).await?;

        return Ok(());
    }

    let user_id = match UserArgs::username(&ctx, &name).await {
        UserArgs::Args(args) => args.user_id,
        UserArgs::User { user, .. } => user.user_id,
        UserArgs::Err(OsuError::NotFound) => {
            let content = if contains_disallowed_infix(&name) {
                format!("`{name}` does not seem to be taken but it likely won't be accepted")
            } else {
                format!("User `{name}` was not found, the name should be available to claim")
            };

            let builder = MessageBuilder::new().embed(content);
            command.update(&ctx, &builder).await?;

            return Ok(());
        }
        UserArgs::Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let args = [
        GameMode::Osu,
        GameMode::Taiko,
        GameMode::Catch,
        GameMode::Mania,
    ]
    .map(|mode| UserArgs::user_id(user_id).mode(mode));

    let user_fut = args
        .into_iter()
        .map(|args| ctx.redis().osu_user(args))
        .collect::<FuturesUnordered<_>>()
        .try_fold(None, |user: Option<User>, next| match user {
            Some(mut user) => {
                next.peek_stats(|stats| match user.statistics {
                    Some(ref mut accum) => accum.playcount += stats.playcount,
                    None => user.statistics = Some(stats.to_owned()),
                });

                future::ready(Ok(Some(user)))
            }
            None => future::ready(Ok(Some(next.into_original()))),
        });

    let user = match user_fut.await {
        Ok(user) => user.unwrap(),
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let embed = ClaimNameEmbed::new(&user, &name).build();
    let builder = MessageBuilder::new().embed(embed);
    command.update(&ctx, &builder).await?;

    Ok(())
}
