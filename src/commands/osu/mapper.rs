use std::{borrow::Cow, sync::Arc};

use command_macros::{command, HasName, SlashCommand};
use eyre::Report;
use hashbrown::HashMap;
use rosu_v2::prelude::OsuError;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::{
        osu::{get_user_and_scores, ScoreArgs, UserArgs},
        GameModeOption,
    },
    core::commands::{prefix::Args, CommandOrigin},
    embeds::{EmbedData, TopEmbed},
    pagination::{Pagination, TopPagination},
    tracking::process_osu_tracking,
    util::{
        builder::MessageBuilder, constants::OSU_API_ISSUE, matcher, numbers, ApplicationCommandExt,
        ChannelExt, CowUtils,
    },
    BotResult, Context,
};

use super::TopScoreOrder;

#[derive(CommandModel, CreateCommand, HasName, SlashCommand)]
#[command(
    name = "mapper",
    help = "Count the top plays on maps of the given mapper.\n\
    It will try to consider guest difficulties so that if a map was created by someone else \
    but the given mapper made the guest diff, it will count.\n\
    Similarly, if the given mapper created the mapset but someone else guest diff'd, \
    it will not count.\n\
    This does not always work perfectly, like when mappers renamed or when guest difficulties don't have \
    common difficulty labels like `X's Insane`"
)]
/// How often does the given mapper appear in top a user's top plays
pub struct Mapper<'a> {
    /// Specify a mapper username
    mapper: Cow<'a, str>,
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<Cow<'a, str>>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

impl<'m> Mapper<'m> {
    fn args(
        mode: Option<GameModeOption>,
        mut args: Args<'m>,
        mapper: Option<&'static str>,
    ) -> Result<Self, &'static str> {
        let mapper = match mapper.or_else(|| args.next()) {
            Some(arg) => arg.into(),
            None => {
                let content = "You need to specify at least one osu! username for the mapper. \
                    If you're not linked, you must specify at least two names.";

                return Err(content);
            }
        };

        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Ok(Self {
            mapper,
            mode,
            name,
            discord,
        })
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username] [mapper]")]
#[examples("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[group(Osu)]
async fn prefix_mapper(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Mapper::args(None, args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a mania user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a mania user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native mania maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[examples("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[alias("mapperm")]
#[group(Mania)]
pub async fn prefix_mappermania(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Mapper::args(Some(GameModeOption::Mania), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a taiko user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a taiko user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native taiko maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[examples("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[alias("mappert")]
#[group(Taiko)]
pub async fn prefix_mappertaiko(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Mapper::args(Some(GameModeOption::Taiko), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a ctb user's top100 are made by the given mapper?")]
#[help(
    "Display the top plays of a ctb user which were mapped by the given mapper.\n\
    Specify the __user first__ and the __mapper second__.\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty.\n\
    If the `-convert` / `-c` argument is specified, I will __not__ count any maps \
    that aren't native ctb maps."
)]
#[usage("[username] [mapper] [-convert]")]
#[example("badewanne3 \"Hishiro Chizuru\"", "monstrata monstrata")]
#[alias("mapperc")]
#[group(Catch)]
async fn prefix_mapperctb(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Mapper::args(Some(GameModeOption::Catch), args, None) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

#[command]
#[desc("How many maps of a user's top100 are made by Sotarks?")]
#[help(
    "How many maps of a user's top100 are made by Sotarks?\n\
    Unlike the mapper count of the profile command, this command considers not only \
    the map's creator, but also tries to check if the map is a guest difficulty."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[group(Osu)]
pub async fn prefix_sotarks(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> BotResult<()> {
    match Mapper::args(Some(GameModeOption::Osu), args, Some("sotarks")) {
        Ok(args) => mapper(ctx, msg.into(), args).await,
        Err(content) => {
            msg.error(&ctx, content).await?;

            Ok(())
        }
    }
}

async fn slash_mapper(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Mapper::from_interaction(command.input_data())?;

    mapper(ctx, command.into(), args).await
}

async fn mapper(ctx: Arc<Context>, orig: CommandOrigin<'_>, args: Mapper<'_>) -> BotResult<()> {
    let (user, mode) = name_mode!(ctx, orig, args);
    let mapper = args.mapper.cow_to_ascii_lowercase();

    // Retrieve the user and their top scores
    let user_args = UserArgs::new(user.as_str(), mode);
    let score_args = ScoreArgs::top(100).with_combo();

    let (mut user, mut scores) = match get_user_and_scores(&ctx, user_args, &score_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{user}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Overwrite default mode
    user.mode = mode;

    // Process user and their top scores for tracking
    process_osu_tracking(&ctx, &mut scores, Some(&user)).await;

    let scores: Vec<_> = scores
        .into_iter()
        .enumerate()
        .filter(|(_, score)| {
            let map = &score.map.as_ref().unwrap();
            let mapset = &score.mapset.as_ref().unwrap();

            //  Filter converts
            if map.mode != mode {
                return false;
            }

            // Either the version contains the mapper name (guest diff'd by mapper)
            // or the map is created by mapper name and not guest diff'd by someone else
            let version = map.version.to_lowercase();

            version.contains(mapper.as_ref())
                || (mapset.creator_name.to_lowercase().as_str() == mapper.as_ref()
                    && !matcher::is_guest_diff(&version))
        })
        .collect();

    // Accumulate all necessary data
    let content = match mapper.as_ref() {
        "sotarks" => {
            let amount = scores.len();

            let mut content = format!(
                "I found {amount} Sotarks map{plural} in `{name}`'s top100, ",
                amount = amount,
                plural = if amount != 1 { "s" } else { "" },
                name = user.username,
            );

            let to_push = match amount {
                0 => "I'm proud \\:)",
                1..=4 => "that's already too many...",
                5..=8 => "kinda sad \\:/",
                9..=15 => "pretty sad \\:(",
                16..=25 => "this is so sad \\:((",
                26..=35 => "this needs to stop this",
                36..=49 => "that's a serious problem...",
                50 => "that's half. HALF.",
                51..=79 => "how do you sleep at night...",
                80..=89 => "so close to ultimate disaster...",
                90..=99 => "i'm not even mad, that's just impressive",
                100 => "you did it. \"Congrats\".",
                _ => "wait how did you do that",
            };

            content.push_str(to_push);

            content
        }
        _ => format!(
            "{} of `{}`'{} top score maps were mapped by `{mapper}`",
            scores.len(),
            user.username,
            if user.username.ends_with('s') {
                ""
            } else {
                "s"
            },
        ),
    };

    let sort_by = TopScoreOrder::Pp;
    let farm = HashMap::new();

    let builder = if scores.is_empty() {
        MessageBuilder::new().embed(content)
    } else {
        let pages = numbers::div_euclid(5, scores.len());

        let embed_fut = TopEmbed::new(
            &user,
            scores.iter().take(5),
            &ctx,
            sort_by,
            &farm,
            (1, pages),
        );

        let data = embed_fut.await;
        let embed = data.build();

        MessageBuilder::new().content(content).embed(embed)
    };

    let response_raw = orig.create_message(&ctx, &builder).await?;

    // Skip pagination if too few entries
    if scores.len() <= 5 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = TopPagination::new(response, user, scores, sort_by, farm, Arc::clone(&ctx));
    let owner = orig.user_id()?;

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
