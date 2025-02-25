use std::borrow::Cow;

use bathbot_macros::{command, HasName, SlashCommand};
use bathbot_model::RelaxPlayersDataResponse;
use bathbot_util::{
    constants::{GENERAL_ISSUE, RELAX},
    numbers::WithComma,
    osu::flag_url,
    AuthorBuilder, EmbedBuilder, MessageBuilder, MessageOrigin,
};
use eyre::{Report, Result};
use rosu_v2::{error::OsuError, model::GameMode, request::UserId};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    commands::osu::require_link,
    core::{commands::CommandOrigin, Context},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(
    name = "relax_profile",
    desc = "Display your relax profile",
    help = "Display your relax profile info"
)]
pub struct RelaxPlayerProfile<'a> {
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

async fn slash_relaxplayerprofile(mut command: InteractionCommand) -> Result<()> {
    let args = RelaxPlayerProfile::from_interaction(command.input_data())?;

    relax_player_profile((&mut command).into(), args).await
}

pub(super) async fn relax_player_profile(
    orig: CommandOrigin<'_>,
    args: RelaxPlayerProfile<'_>,
) -> Result<()> {
    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let (user_id, _) = match user_id!(orig, args) {
        Some(user_id) => (user_id, false),
        None => match config.osu {
            Some(user_id) => (UserId::Id(user_id), true),
            None => return require_link(&orig).await,
        },
    };
    let user_args = UserArgs::rosu_id(&user_id, GameMode::Osu).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let client = Context::client();

    let user_id = user.user_id.to_native();

    let info_fut = client.get_relax_player(user_id);
    let relax_player = info_fut.await?;

    if let None = relax_player {
        return orig
            .error(format!("User `{}` not found", user.username))
            .await;
    }

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());
    let mut pagination = RelaxProfile::new(user, relax_player.unwrap(), origin);

    let builder = MessageBuilder::new().embed(pagination.compact().unwrap());
    orig.create_message(builder).await?;

    Ok(())
}

pub struct RelaxProfile {
    user: CachedUser,
    info: RelaxPlayersDataResponse,
    origin: MessageOrigin,
}

impl RelaxProfile {
    pub fn new(user: CachedUser, info: RelaxPlayersDataResponse, origin: MessageOrigin) -> Self {
        Self { user, info, origin }
    }

    pub fn compact(&mut self) -> Result<EmbedBuilder> {
        let stats = &self.info;
        let description = format!(
            "Accuracy: [`{acc:.2}%`]({origin} \"{acc}\")\n\
            Playcount: `{playcount}`",
            acc = stats.total_accuracy.unwrap_or_default(),
            origin = self.origin,
            playcount = WithComma::new(stats.playcount),
        );

        let embed = EmbedBuilder::new()
            .author(self.author_builder())
            .description(description)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(embed)
    }

    fn author_builder(&self) -> AuthorBuilder {
        let country_code = self.user.country_code.as_str();
        let pp = self.info.total_pp;

        let text = format!(
            "{name}: {pp}pp (#{rank}, {country_code}{country_rank})",
            name = self.user.username,
            pp = WithComma::new(pp.unwrap()),
            rank = self.info.rank.unwrap_or_default(),
            country_rank = self.info.country_rank.unwrap_or_default(),
        );

        let url = format!("{RELAX}/users/{}", self.user.user_id);
        let icon = flag_url(country_code);
        AuthorBuilder::new(text).url(url).icon_url(icon)
    }
}
