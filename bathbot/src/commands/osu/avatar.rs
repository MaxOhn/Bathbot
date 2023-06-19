use std::{borrow::Cow, sync::Arc};

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSU_API_ISSUE, OSU_BASE},
    matcher,
    osu::flag_url,
    AuthorBuilder, EmbedBuilder, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{prelude::OsuError, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::{require_link, user_not_found};
use crate::{
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(name = "avatar", desc = "Display someone's osu! profile picture")]
pub struct Avatar<'a> {
    #[command(desc = "Specify a username")]
    name: Option<Cow<'a, str>>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

pub async fn slash_avatar(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Avatar::from_interaction(command.input_data())?;

    avatar(ctx, (&mut command).into(), args).await
}

#[command]
#[desc("Display someone's osu! profile picture")]
#[alias("pfp")]
#[usage("[username]")]
#[example("Badewanne3")]
#[group(AllModes)]
async fn prefix_avatar(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    avatar(ctx, msg.into(), Avatar::args(args)).await
}

impl<'m> Avatar<'m> {
    fn args(mut args: Args<'m>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self { name, discord }
    }
}

async fn avatar(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Avatar<'_>) -> Result<()> {
    let user_id = match user_id!(ctx, orig, args) {
        Some(user_id) => user_id,
        None => match ctx.user_config().osu_id(orig.user_id()?).await {
            Ok(Some(user_id)) => UserId::Id(user_id),
            Ok(None) => return require_link(&ctx, &orig).await,
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await;

    let user = match ctx.redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(OsuError::NotFound) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
    };

    let embed = match user {
        RedisData::Original(user) => {
            let author = AuthorBuilder::new(user.username.into_string())
                .url(format!("{OSU_BASE}u/{}", user.user_id))
                .icon_url(flag_url(&user.country_code));

            EmbedBuilder::new().author(author).image(user.avatar_url)
        }
        RedisData::Archive(user) => {
            let author = AuthorBuilder::new(user.username.as_str())
                .url(format!("{OSU_BASE}u/{}", user.user_id))
                .icon_url(flag_url(user.country_code.as_str()));

            EmbedBuilder::new()
                .author(author)
                .image(user.avatar_url.as_ref())
        }
    };

    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}
