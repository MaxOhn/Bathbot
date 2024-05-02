use std::{borrow::Cow, fmt::Write};

use bathbot_model::Countries;
use bathbot_util::{
    constants::GENERAL_ISSUE,
    matcher,
    osu::{MapIdType, ModSelection},
    CowUtils, MessageBuilder,
};
use eyre::Result;
use rosu_v2::prelude::{GameMode, Grade, RankStatus};
use twilight_interactions::command::AutocompleteValue;

use super::{get_mode, process_scores, separate_content, MapScores, ScoresOrder};
use crate::{
    active::{impls::ScoresMapPagination, ActiveMessages},
    commands::osu::{
        compare::{slash_compare_score, ScoreOrder},
        CompareScoreAutocomplete, HasMods, ModsResult,
    },
    core::Context,
    util::{
        interaction::InteractionCommand,
        query::{FilterCriteria, IFilterCriteria, ScoresCriteria},
        Authored, CheckPermissions, InteractionCommandExt,
    },
};

pub async fn map_scores(mut command: InteractionCommand, args: MapScores) -> Result<()> {
    let Some(guild_id) = command.guild_id else {
        // TODO: use mode when /cs uses it
        let MapScores {
            map,
            mode: _,
            sort,
            mods,
            country: _,
            query: _,
            per_user: _,
            index,
            reverse: _,
            grade: _,
        } = args;

        let sort = match sort {
            Some(ScoresOrder::Acc) => Some(ScoreOrder::Acc),
            Some(ScoresOrder::Combo) => Some(ScoreOrder::Combo),
            Some(ScoresOrder::Date) => Some(ScoreOrder::Date),
            Some(ScoresOrder::Misses) => Some(ScoreOrder::Misses),
            Some(ScoresOrder::Pp) => Some(ScoreOrder::Pp),
            Some(ScoresOrder::Score) => Some(ScoreOrder::Score),
            Some(ScoresOrder::Stars) => Some(ScoreOrder::Stars),
            None => None,
            Some(
                ScoresOrder::Ar
                | ScoresOrder::Bpm
                | ScoresOrder::Cs
                | ScoresOrder::Hp
                | ScoresOrder::Length
                | ScoresOrder::Od
                | ScoresOrder::RankedDate,
            ) => {
                let content = "When using this command in DMs, \
                the only available sort orders are \
                `Accuracy`, `Combo`, `Date`, `Misses`, `PP`, `Score`, or `Stars`";
                command.error(content).await?;

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

        return slash_compare_score(&mut command, args).await;
    };

    let mods = match args.mods() {
        ModsResult::Mods(mods) => Some(mods),
        ModsResult::None => None,
        ModsResult::Invalid => {
            let content = "Failed to parse mods.\n\
                If you want included mods, specify it e.g. as `+hrdt`.\n\
                If you want exact mods, specify it e.g. as `+hdhr!`.\n\
                And if you want to exclude mods, specify it e.g. as `-hdnf!`.";

            command.error(content).await?;

            return Ok(());
        }
    };

    let map = match args.map {
        Some(ref map) => {
            let map_opt = matcher::get_osu_map_id(map)
                .map(MapIdType::Map)
                .or_else(|| matcher::get_osu_mapset_id(map).map(MapIdType::Set));

            if map_opt.is_none() {
                let content =
                    "Failed to parse map url. Be sure you specify a valid map id or url to a map.";
                command.error(content).await?;

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
            command.error(content).await?;

            return Ok(());
        }
        None if command.can_read_history() => {
            let msgs = match Context::retrieve_channel_history(command.channel_id()).await {
                Ok(msgs) => msgs,
                Err(err) => {
                    let _ = command.error(GENERAL_ISSUE).await;

                    return Err(err.wrap_err("Failed to retrieve channel history"));
                }
            };

            match Context::find_map_id_in_msgs(&msgs, 0).await {
                Some(MapIdType::Map(id)) => id,
                None | Some(MapIdType::Set(_)) => {
                    let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map either by url to the map, or just by map id.";
                    command.error(content).await?;

                    return Ok(());
                }
            }
        }
        None => {
            let content =
                "No beatmap specified and lacking permission to search the channel history for maps.\n\
                Try specifying a map either by url to the map, or just by map id, \
                or give me the \"Read Message History\" permission.";

            command.error(content).await?;

            return Ok(());
        }
    };

    let country_code = match args.country {
        Some(ref country) => match Countries::name(country).to_code() {
            Some(code) => Some(Cow::Borrowed(code)),
            None if country.len() == 2 => Some(country.cow_to_ascii_uppercase()),
            None => {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(content).await?;

                return Ok(());
            }
        },
        None => None,
    };

    let cache = Context::cache();
    let guild_fut = cache.guild(guild_id);
    let members_fut = cache.members(guild_id);
    let owner = command.user_id()?;
    let mode_fut = get_mode(args.mode, owner);

    let (guild_res, members_res, mode_res) = tokio::join!(guild_fut, members_fut, mode_fut);

    let guild_icon = guild_res
        .ok()
        .flatten()
        .and_then(|guild| Some((guild.id, *guild.icon.as_ref()?)));

    let members: Vec<_> = match members_res {
        Ok(members) => members.into_iter().map(|id| id as i64).collect(),
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let mode = mode_res.unwrap_or_else(|err| {
        warn!(?err);

        None
    });

    let grade = args.grade.map(Grade::from);

    let scores_fut = Context::osu_scores().from_discord_ids(
        &members,
        mode,
        mods.as_ref(),
        country_code.as_deref(),
        Some(map_id),
        grade,
    );

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

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
        command.update(builder).await?;

        return Ok(());
    } else if scores.maps().next().zip(scores.mapsets().next()).is_none() {
        let content = format!("Looks like I don't have map id {map_id} stored");
        let builder = MessageBuilder::new().embed(content);
        command.update(builder).await?;

        return Ok(());
    }

    let sort = match args.sort {
        Some(sort) => sort,
        None => match scores.mapsets().next() {
            Some((_, mapset)) => match mapset.rank_status {
                RankStatus::Ranked | RankStatus::Approved => ScoresOrder::Pp,
                _ => ScoresOrder::Score,
            },
            None => Default::default(),
        },
    };

    let criteria = args.query.as_deref().map(ScoresCriteria::create);

    let content = msg_content(
        sort,
        mods.as_ref(),
        country_code.as_deref(),
        grade,
        criteria.as_ref(),
    );

    process_scores(
        &mut scores,
        None,
        sort,
        None,
        criteria.as_ref(),
        args.per_user,
        args.reverse,
    );

    let pagination = ScoresMapPagination::builder()
        .scores(scores)
        .mode(mode)
        .sort(sort)
        .guild_icon(guild_icon)
        .content(content.into_boxed_str())
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}

fn msg_content(
    sort: ScoresOrder,
    mods: Option<&ModSelection>,
    country: Option<&str>,
    grade: Option<Grade>,
    criteria: Option<&FilterCriteria<ScoresCriteria<'_>>>,
) -> String {
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

    if let Some(country) = country {
        separate_content(&mut content);
        let _ = write!(content, "`Country: {country}`");
    }

    if let Some(grade) = grade {
        separate_content(&mut content);
        let _ = write!(content, "`Grade: {grade:?}`");
    }

    if let Some(criteria) = criteria {
        criteria.display(&mut content);
    }

    separate_content(&mut content);

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
