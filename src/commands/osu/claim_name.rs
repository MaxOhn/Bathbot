use std::sync::Arc;

use command_macros::SlashCommand;
use futures::{stream::FuturesUnordered, TryStreamExt};
use rosu_v2::prelude::{GameMode, OsuError, User};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    core::Context,
    embeds::ClaimNameEmbed,
    embeds::EmbedData,
    util::{self, builder::MessageBuilder, constants::OSU_API_ISSUE, ApplicationCommandExt},
    BotResult,
};

use super::UserArgs;

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

async fn slash_claimname(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let ClaimName { name } = ClaimName::from_interaction(command.input_data())?;

    let args = [GameMode::STD, GameMode::TKO, GameMode::CTB, GameMode::MNA]
        .map(|mode| UserArgs::new(&name, mode));

    let redis = ctx.redis();

    let user_fut = args
        .iter()
        .map(|args| redis.osu_user(&args))
        .collect::<FuturesUnordered<_>>()
        .try_fold(None, |mut user: Option<User>, next| {
            match user.as_mut() {
                Some(user) => {
                    if let Some(next) = next.statistics {
                        match user.statistics {
                            Some(ref mut accum) => accum.playcount += next.playcount,
                            None => user.statistics = Some(next),
                        }
                    }
                }
                None => user = Some(next),
            }

            futures::future::ready(Ok(user))
        });

    let user = match user_fut.await {
        Ok(user) => user.unwrap(),
        Err(OsuError::NotFound) => {
            let content = if util::contains_disallowed_infix(&name) {
                format!("`{name}` does not seem to be taken but it likely won't be accepted")
            } else {
                format!("User `{name}` was not found, the name should be available to claim")
            };

            let builder = MessageBuilder::new().embed(content);
            command.update(&ctx, &builder).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    let embed = ClaimNameEmbed::new(&user, &name).build();
    let builder = MessageBuilder::new().embed(embed);
    command.update(&ctx, &builder).await?;

    Ok(())
}
