use std::{borrow::Cow, fmt::Write, sync::Arc};

use bathbot_util::{
    constants::GENERAL_ISSUE,
    matcher,
    osu::{MapIdType, ModSelection},
    MessageBuilder,
};
use eyre::Result;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::AutocompleteValue;

use super::{process_scores, MapScores, ScoresOrder};
use crate::{
    commands::osu::{
        compare::{slash_compare_score, ScoreOrder},
        CompareScoreAutocomplete, HasMods, ModsResult,
    },
    core::Context,
    pagination::MapScoresPagination,
    util::{interaction::InteractionCommand, Authored, CheckPermissions, InteractionCommandExt},
};

pub async fn map_scores(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: MapScores,
) -> Result<()> {
    let Some(guild_id) = command.guild_id else {
        // TODO: use mode when /cs uses it
        let MapScores { map, mode: _, sort, mods, index, reverse:_ } = args;

        let sort = match sort {
            Some(ScoresOrder::Acc) => Some(ScoreOrder::Acc),
            Some(ScoresOrder::Combo) => Some(ScoreOrder::Combo),
            Some(ScoresOrder::Date) => Some(ScoreOrder::Date),
            Some(ScoresOrder::Misses) => Some(ScoreOrder::Misses),
            Some(ScoresOrder::Pp) => Some(ScoreOrder::Pp),
            Some(ScoresOrder::Score) => Some(ScoreOrder::Score),
            Some(ScoresOrder::Stars) => Some(ScoreOrder::Stars),
            None => None,
            Some(ScoresOrder::Ar |
            ScoresOrder::Bpm |
            ScoresOrder::Cs |
            ScoresOrder::Hp |
            ScoresOrder::Length |
            ScoresOrder::Od |
            ScoresOrder::RankedDate) => {
                let content = "When using this command in DMs, \
                the only available sort orders are \
                `Accuracy`, `Combo`, `Date`, `Misses`, `PP`, `Score`, or `Stars`";
                command.error(&ctx, content).await?;

                return Ok(());
            }
        };

        let args = CompareScoreAutocomplete {
            name: None,
            map: map.map(Cow::Owned),
            difficulty: AutocompleteValue::None,
            sort,
            mods: mods.map(Cow::Owned),
            index,
            discord: None,
        };

        return slash_compare_score(ctx, &mut command, args).await;
    };

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let mode = args.mode.map(GameMode::from);

    let map = match args.map {
        Some(ref map) => {
            let map_opt = matcher::get_osu_map_id(map)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(map).map(MapIdType::Set));

            if map_opt.is_none() {
                let content =
                    "Failed to parse map url. Be sure you specify a valid map id or url to a map.";
                command.error(&ctx, content).await?;

                return Ok(());
            }

            map_opt
        }
        None => None,
    };

    let map_id = match map {
        Some(MapIdType::Map(id)) => id,
        Some(MapIdType::Set(_)) => {
            let content = "Looks like you gave me a mapset id, I need a map id though";
            command.error(&ctx, content).await?;

            return Ok(());
        }
        None if command.can_read_history() => {
            let msgs = match ctx.retrieve_channel_history(command.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = command.error(&ctx, GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to retrieve channel history"));
                }
            };

            match MapIdType::map_from_msgs(&msgs, 0) {
                Some(id) => id,
                None => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";
                    command.error(&ctx, content).await?;

                    return Ok(());
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            command.error(&ctx, content).await?;

            return Ok(());
        }
    };

    let guild_fut = ctx.cache.guild(guild_id);
    let members_fut = ctx.cache.members(guild_id);

    let (guild_res, members_res) = tokio::join!(guild_fut, members_fut);

    let guild_icon = guild_res
        .ok()
        .flatten()
        .and_then(|guild| Some((guild.id, *guild.icon.as_ref()?)));

    let members: Vec<_> = match members_res {
        Ok(members) => members.into_iter().map(|id| id as i64).collect(),
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let scores_fut =
        ctx.osu_scores()
            .from_discord_ids(&members, mode, mods.as_ref(), None, Some(map_id));

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    if scores.is_empty() {
        let content = format!(
            "Looks like I don't have any stored{mode} scores on map id {map_id}{mods}",
            mode = match mode {
                Some(GameMode::Osu) => " osu!",
                Some(GameMode::Taiko) => " taiko",
                Some(GameMode::Catch) => " catch",
                Some(GameMode::Mania) => " mania",
                None => "",
            },
            mods = match mods {
                Some(_) => " for the specified mods",
                None => "",
            }
        );
        let builder = MessageBuilder::new().embed(content);
        command.update(&ctx, &builder).await?;

        return Ok(());
    } else if scores.maps().next().zip(scores.mapsets().next()).is_none() {
        let content = format!("Looks like I don't have map id {map_id} stored");
        let builder = MessageBuilder::new().embed(content);
        command.update(&ctx, &builder).await?;

        return Ok(());
    }

    let sort = args.sort.unwrap_or_default();
    let content = msg_content(sort, mods.as_ref());
    process_scores(&mut scores, None, sort, args.reverse);

    MapScoresPagination::builder(scores, mode, sort, guild_icon)
        .content(content)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}

fn msg_content(sort: ScoresOrder, mods: Option<&ModSelection>) -> String {
    let mut content = String::new();

    match mods {
        Some(ModSelection::Include(mods)) => {
            let _ = write!(content, "`Mods: Include {mods}`");
        }
        Some(ModSelection::Exclude(mods)) => {
            let _ = write!(content, "`Mods: Exclude {mods}`");
        }
        Some(ModSelection::Exact(mods)) => {
            let _ = write!(content, "`Mods: {mods}`");
        }
        None => {}
    }

    if !content.is_empty() {
        content.push_str(" • ");
    }

    content.push_str("`Order: ");

    let order = match sort {
        ScoresOrder::Acc => "Accuracy",
        ScoresOrder::Ar => "AR",
        ScoresOrder::Bpm => "BPM",
        ScoresOrder::Combo => "Combo",
        ScoresOrder::Cs => "CS",
        ScoresOrder::Date => "Date",
        ScoresOrder::Hp => "HP",
        ScoresOrder::Length => "Length",
        ScoresOrder::Misses => "Miss count",
        ScoresOrder::Od => "OD",
        ScoresOrder::Pp => "PP",
        ScoresOrder::RankedDate => "Ranked date",
        ScoresOrder::Score => "Score",
        ScoresOrder::Stars => "Stars",
    };

    content.push_str(order);
    content.push('`');

    content
}
