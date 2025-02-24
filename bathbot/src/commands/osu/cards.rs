use std::collections::HashMap;

use bathbot_cards::{BathbotCard, RequiredAttributes};
use bathbot_macros::{HasName, SlashCommand};
use bathbot_model::command_fields::GameModeOption;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    EmbedBuilder, IntHasher, MessageBuilder,
    constants::{GENERAL_ISSUE, OSEKAI_ISSUE},
    datetime::DATE_FORMAT,
    osu::flag_url_size,
};
use eyre::{Report, Result, WrapErr};
use futures::{TryStreamExt, stream::FuturesUnordered};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{Id, marker::UserMarker};

use super::{require_link, user_not_found};
use crate::{
    core::{BotConfig, Context, commands::CommandOrigin},
    embeds::attachment,
    manager::redis::osu::{UserArgs, UserArgsError},
    util::{CachedUserExt, InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(
    name = "card",
    desc = "Create a user card",
    help = "Create a visual user card containing various fun values about the user.\n\
    Most skill values are based on the strain value of the official pp calculation. \
    Only the accuracy values for [catch](https://www.desmos.com/calculator/cg59pywpry) \
    and [mania](https://www.desmos.com/calculator/b30p1awwft) come from custom formulas \
    that are based on score accuracy, map OD, object count, and star rating.\n\
    Note that only the user's top100 is considered while calculating card values.\n\
    Titles consist of three parts: **prefix**, **descriptions**, and **suffix**.\n\n\
    - The **prefix** is determined by checking the highest skill value \
    for thresholds:\n\
    ```\n\
    - <10: Newbie      | - <70: Seasoned\n\
    - <20: Novice      | - <80: Professional\n\
    - <30: Rookie      | - <85: Expert\n\
    - <40: Apprentice  | - <90: Master\n\
    - <50: Advanced    | - <95: Legendary\n\
    - <60: Outstanding | - otherwise: God\n\
    ```\n\
    - The **descriptions** are determined by counting properties in top scores:\n  \
    - `>70 NM`: `Mod-Hating`\n  \
    - `>60 DT / NC`: `Speedy`\n  \
    - `>30 HT`: `Slow-Mo`\n  \
    - `>15 FL`: `Blindsighted`\n  \
    - `>20 SO`: `Lazy-Spin`\n  \
    - `>60 HD`: `HD-Abusing` / `Ghost-Fruits` / `Brain-Lag`\n  \
    - `>60 HR`: `Ant-Clicking` / `Zooming` / `Pea-Catching`\n  \
    - `>15 EZ`: `Patient` / `Training-Wheels` / `3-Life`\n  \
    - `>30 MR`: `Unmindblockable`\n  \
    - none of above but `<10 NM`: `Mod-Loving`\n  \
    - none of above: `Versatile`\n  \
    - `<50 CL`: `New-Skool`\n  \
    - `>70 Key[X]`: `[X]K`\n  \
    - otherwise: `Multi-Key`\n\
    - The **suffix** is determined by checking proximity of skill \
    values to each other:\n  \
    - osu!:\n    \
    - All skills are roughly the same: `All-Rounder`\n    \
    - High accuracy and aim but low speed: `Sniper`\n    \
    - High accuracy and speed but low aim: `Ninja`\n    \
    - High aim and speed but low accuracy: `Gunslinger`\n    \
    - Only high accuracy: `Rhythm Enjoyer`\n    \
    - Only high aim: `Whack-A-Mole`\n    \
    - Only high speed: `Masher`\n  \
    - taiko, catch, and mania:\n    \
    - All skills are roughly the same: `Gamer`\n    \
    - High accuracy but low strain: `Rhythm Enjoyer`\n    \
    - High strain but low accuracy: `Masher` / `Droplet Dodger`"
)]
pub struct Card {
    #[command(desc = "Specify a gamemode")]
    mode: Option<GameModeOption>,
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

async fn slash_card(mut command: InteractionCommand) -> Result<()> {
    let args = Card::from_interaction(command.input_data())?;

    let orig = CommandOrigin::Interaction {
        command: &mut command,
    };

    let owner = orig.user_id()?;
    let config = Context::user_config().with_osu_id(owner).await?;

    let user_id = match user_id!(orig, args) {
        Some(user_id) => user_id,
        None => match config.osu {
            Some(user_id) => UserId::Id(user_id),
            None => return require_link(&orig).await,
        },
    };

    let mode = args
        .mode
        .map(GameMode::from)
        .or(config.mode)
        .unwrap_or(GameMode::Osu);

    let legacy_scores = match config.score_data {
        Some(score_data) => score_data.is_legacy(),
        None => match orig.guild_id() {
            Some(guild_id) => Context::guild_config()
                .peek(guild_id, |config| config.score_data)
                .await
                .is_some_and(ScoreData::is_legacy),
            None => false,
        },
    };

    let user_args = UserArgs::rosu_id(&user_id, mode).await;
    let scores_fut = Context::osu_scores()
        .top(legacy_scores)
        .limit(100)
        .exec_with_user(user_args);
    let medals_fut = Context::redis().medals();

    let (user, scores, total_medals) = match tokio::join!(scores_fut, medals_fut) {
        (Ok((user, scores)), Ok(medals)) => (user, scores, medals.len()),
        (Err(UserArgsError::Osu(OsuError::NotFound)), _) => {
            let content = user_not_found(user_id).await;

            return orig.error(content).await;
        }
        (Err(err), _) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
        (_, Err(err)) => {
            let _ = orig.error(OSEKAI_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached medals"));
        }
    };

    if scores.is_empty() {
        let content = "Looks like they don't have any scores on that mode";
        orig.error(content).await?;

        return Ok(());
    }

    let maps: HashMap<_, _, IntHasher> = scores
        .iter()
        .map(|score| async {
            let map = Context::osu_map()
                .pp_map(score.map_id)
                .await
                .wrap_err("Failed to get pp map")?;

            let difficulty = Context::pp_parsed(&map, mode)
                .lazer(score.set_on_lazer)
                .mods(score.mods.clone())
                .difficulty()
                .await
                .to_owned();

            let attrs = RequiredAttributes {
                difficulty,
                od: map.od,
            };

            Ok::<_, Report>((score.map_id, attrs))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await?;

    let client = Context::client();
    let pfp_fut = client.get_avatar(user.avatar_url.as_ref());
    let flag_url = flag_url_size(user.country_code.as_str(), 70);
    let flag_fut = client.get_flag(&flag_url);

    let (pfp, flag) = match tokio::join!(pfp_fut, flag_fut) {
        (Ok(pfp), Ok(flag)) => (pfp, flag),
        (Err(err), _) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to acquire card avatar"));
        }
        (_, Err(err)) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to acquire card flag"));
        }
    };

    let stats = user.statistics.as_ref().expect("missing stats");

    let medals = user.medals.len();

    let today = OffsetDateTime::now_utc()
        .date()
        .format(DATE_FORMAT)
        .unwrap();

    let card_res = BathbotCard::new(mode, &scores, maps, legacy_scores)
        .user(user.username.as_str(), stats.level.float())
        .ranks(
            stats.global_rank.to_native(),
            stats.country_rank.to_native(),
        )
        .medals(medals as u32, total_medals as u32)
        .bytes(&pfp, &flag)
        .date(&today)
        .assets(BotConfig::get().paths.assets.clone())
        .draw();

    let bytes = match card_res {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = orig.error("Failed to draw the card :(").await;

            return Err(Report::new(err).wrap_err("Failed to draw card"));
        }
    };

    let embed = EmbedBuilder::new()
        .author(user.author_builder(false))
        .image(attachment("card.png"));

    let builder = MessageBuilder::new()
        .attachment("card.png", bytes)
        .embed(embed);

    orig.create_message(builder).await?;

    Ok(())
}
