use std::fmt::Write;

use bathbot_model::RelaxPlayersDataResponse;
use bathbot_util::{
    AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder, MessageOrigin,
    constants::{GENERAL_ISSUE, RELAX},
    datetime::NAIVE_DATETIME_FORMAT,
    fields,
    numbers::WithComma,
    osu::flag_url,
};
use eyre::{Report, Result};
use rosu_v2::{
    error::OsuError,
    model::{GameMode, Grade},
    request::UserId,
};
use twilight_model::id::{Id, marker::UserMarker};

use crate::{
    commands::osu::require_link,
    core::{Context, commands::CommandOrigin},
    manager::redis::osu::{CachedUser, UserArgs, UserArgsError},
    util::osu::grade_emote,
};

use crate::commands::osu::relax::RelaxProfile;

pub(super) async fn relax_profile(orig: CommandOrigin<'_>, args: RelaxProfile<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(msg_owner).await?;

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
    let user_id = user.user_id.to_native();
    let client = Context::client();
    let info_fut = client.get_relax_player(user_id);

    let guild = orig.guild_id();
    let user_id_fut = Context::user_config().discord_from_osu_id(user_id);

    let (info_res, user_id_res) = tokio::join!(info_fut, user_id_fut);
    let discord_id = match user_id_res {
        Ok(user) => match (guild, user) {
            (Some(guild), Some(user)) => Context::cache()
                .member(guild, user) // make sure the user is in the guild
                .await?
                .map(|_| user),
            _ => None,
        },
        Err(err) => {
            warn!(?err, "Failed to get discord id from osu user id");

            None
        }
    };

    let info_res = info_res?;
    if let None = info_res {
        return orig
            .error(format!("User `{}` not found", user.username))
            .await;
    }

    let origin = MessageOrigin::new(orig.guild_id(), orig.channel_id());
    let pagination = RelaxProfileArgs::new(user, discord_id, info_res.unwrap(), origin, msg_owner);

    let builder = MessageBuilder::new().embed(relax_profile_builder(pagination).unwrap());
    orig.create_message(builder).await?;

    Ok(())
}

pub struct RelaxProfileArgs {
    user: CachedUser,
    discord_id: Option<Id<UserMarker>>,
    info: RelaxPlayersDataResponse,
    origin: MessageOrigin,
    msg_owner: Id<UserMarker>,
}

impl RelaxProfileArgs {
    pub fn new(
        user: CachedUser,
        discord_id: Option<Id<UserMarker>>,
        info: RelaxPlayersDataResponse,
        origin: MessageOrigin,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            user,
            discord_id,
            info,
            origin,
            msg_owner,
        }
    }
}
pub fn relax_profile_builder(args: RelaxProfileArgs) -> Result<EmbedBuilder> {
    let stats = &args.info;
    let mut description = format!("__**Relax user statistics");
    if let Some(discord_id) = args.discord_id {
        let _ = write!(description, "for <@{discord_id}>");
    };

    description.push_str(":**__");
    let _ = writeln!(
        description,
        "\n
        Accuracy: [`{acc:.2}%`]({origin} \"{acc}\") • \
        Playcount: `{playcount}`",
        origin = args.origin,
        acc = stats.total_accuracy.unwrap_or_default(),
        playcount = WithComma::new(stats.playcount)
    );
    let ss_grades = format!("{}{}", stats.count_ss, grade_emote(Grade::X));
    let s_grades = format!("{}{}", stats.count_s, grade_emote(Grade::S));
    let a_grades = format!("{}{}", stats.count_a, grade_emote(Grade::A));
    let fields = fields![
        "Count SS", ss_grades, true;
        "Count S", s_grades, true;
        "Count A", a_grades, true;
    ];
    let embed = EmbedBuilder::new()
        .author(relax_author_builder(&args))
        .description(description)
        .fields(fields)
        .thumbnail(args.user.avatar_url.as_ref())
        .footer(relax_footer_builder(&args));

    Ok(embed)
}

fn relax_author_builder(args: &RelaxProfileArgs) -> AuthorBuilder {
    let country_code = args.user.country_code.as_str();
    let pp = args.info.total_pp;

    let text = format!(
        "{name}: {pp}pp (#{rank} {country_code}{country_rank})",
        name = args.user.username,
        pp = WithComma::new(pp.unwrap()),
        rank = args.info.rank.unwrap_or_default(),
        country_rank = args.info.country_rank.unwrap_or_default(),
    );

    let url = format!("{RELAX}/users/{}", args.user.user_id);
    let icon = flag_url(country_code);
    AuthorBuilder::new(text).url(url).icon_url(icon)
}

fn relax_footer_builder(args: &RelaxProfileArgs) -> FooterBuilder {
    let last_update = format!(
        "Last update: {}",
        args.info
            .updated_at
            .unwrap()
            .format(NAIVE_DATETIME_FORMAT)
            .unwrap()
    );
    FooterBuilder::new(last_update).icon_url("https://rx.stanr.info/rv-yellowlight-192.png")
}
