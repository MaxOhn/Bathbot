use std::{collections::HashMap, sync::Arc};

use bathbot_cards::BathbotCard;
use bathbot_macros::{HasName, SlashCommand};
use bathbot_util::{
    constants::{GENERAL_ISSUE, OSEKAI_ISSUE, OSU_API_ISSUE},
    datetime::DATE_FORMAT,
    osu::flag_url_size,
    EmbedBuilder, IntHasher, MessageBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::{stream::FuturesUnordered, TryStreamExt};
use rosu_v2::prelude::OsuError;
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use super::user_not_found;
use crate::{
    commands::GameModeOption,
    core::{commands::CommandOrigin, BotConfig, Context},
    embeds::attachment,
    manager::redis::{osu::UserArgs, RedisData},
    util::{interaction::InteractionCommand, InteractionCommandExt},
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

async fn slash_card(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Card::from_interaction(command.input_data())?;

    let orig = CommandOrigin::Interaction {
        command: &mut command,
    };

    let (user_id, mode) = user_id_mode!(ctx, orig, args);

    let user_args = UserArgs::rosu_id(&ctx, &user_id).await.mode(mode);
    let scores_fut = ctx.osu_scores().top().limit(100).exec_with_user(user_args);
    let medals_fut = ctx.redis().medals();

    let (user, scores, total_medals) = match tokio::join!(scores_fut, medals_fut) {
        (Ok((user, scores)), Ok(medals)) => {
            let medals_len = match medals {
                RedisData::Original(medals) => medals.len(),
                RedisData::Archive(medals) => medals.len(),
            };

            (user, scores, medals_len)
        }
        (Err(OsuError::NotFound), _) => {
            let content = user_not_found(&ctx, user_id).await;

            return orig.error(&ctx, content).await;
        }
        (Err(err), _) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("failed to get user");

            return Err(err);
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    if scores.is_empty() {
        let content = "Looks like they don't have any scores on that mode";
        orig.error(&ctx, content).await?;

        return Ok(());
    }

    let maps: HashMap<_, _, IntHasher> = scores
        .iter()
        .map(|score| async {
            let map = ctx
                .osu_map()
                .pp_map(score.map_id)
                .await
                .wrap_err("failed to get pp map")?;

            let attrs = ctx
                .pp_parsed(&map, score.map_id, false, mode)
                .mods(&score.mods)
                .difficulty()
                .await
                .to_owned();

            Ok::<_, Report>((score.map_id, (map, attrs)))
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await?;

    let pfp_fut = ctx.client().get_avatar(user.avatar_url());
    let flag_url = flag_url_size(user.country_code(), 70);
    let flag_fut = ctx.client().get_flag(&flag_url);

    let (pfp, flag) = match tokio::join!(pfp_fut, flag_fut) {
        (Ok(pfp), Ok(flag)) => (pfp, flag),
        (Err(err), _) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to acquire card avatar"));
        }
        (_, Err(err)) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.wrap_err("Failed to acquire card flag"));
        }
    };

    let stats = user.stats();

    let medals = match user {
        RedisData::Original(ref user) => user.medals.len(),
        RedisData::Archive(ref user) => user.medals.len(),
    };

    let today = OffsetDateTime::now_utc()
        .date()
        .format(DATE_FORMAT)
        .unwrap();

    let card_res = BathbotCard::new(mode, &scores, maps)
        .user(user.username(), stats.level().float())
        .ranks(stats.global_rank(), stats.country_rank())
        .medals(medals as u32, total_medals as u32)
        .bytes(&pfp, &flag)
        .date(&today)
        .assets(BotConfig::get().paths.assets.clone())
        .draw();

    let bytes = match card_res {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = orig.error(&ctx, "Failed to draw the card :(").await;

            return Err(Report::new(err).wrap_err("Failed to draw card"));
        }
    };

    let embed = EmbedBuilder::new()
        .author(user.author_builder())
        .image(attachment("card.png"));

    let builder = MessageBuilder::new()
        .attachment("card.png", bytes)
        .embed(embed);

    orig.create_message(&ctx, builder).await?;

    Ok(())
}
