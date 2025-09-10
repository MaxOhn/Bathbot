use std::{collections::HashMap, fmt::Write};

use bathbot_macros::command;
use bathbot_util::{
    AuthorBuilder, FooterBuilder, IntHasher, ModsFormatter, constants::GENERAL_ISSUE,
};
use eyre::{Report, Result};
use rkyv::{
    Deserialize,
    rancor::{BoxedError, Strategy},
};
use rosu_v2::{
    model::{GameMode, Grade},
    prelude::{GameMods, RoomLeaderboardItem},
};
use time::{Date, OffsetDateTime, UtcDateTime, macros::date};
use twilight_model::guild::Permissions;

use crate::{
    active::{ActiveMessages, impls::DailyChallengeTodayPagination},
    commands::osu::daily_challenge::DC_TODAY_DESC,
    core::{Context, commands::CommandOrigin},
    manager::OsuMap,
    util::{Emote, osu::MapInfo},
};

#[command]
#[desc(DC_TODAY_DESC)]
#[aliases("dc", "dct", "dctoday", "dailychallengetoday")]
#[group(AllModes)]
async fn prefix_dailychallenge(
    msg: &Message,
    _: Args<'_>,
    perms: Option<Permissions>,
) -> Result<()> {
    today(CommandOrigin::from_msg(msg, perms)).await
}

pub(super) async fn today(orig: CommandOrigin<'_>) -> Result<()> {
    let owner = orig.user_id()?;

    let osu_id = match Context::user_config().osu_id(owner).await {
        Ok(osu_id) => osu_id,
        Err(err) => {
            warn!(%owner, ?err, "Failed to fetch osu id");

            None
        }
    };

    let today = match DailyChallengeDay::new(osu_id, UtcDateTime::now().date()).await {
        Ok(day) => day,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err);
        }
    };

    let pagination = DailyChallengeTodayPagination::new(osu_id, today, owner);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

pub struct DailyChallengeDay {
    pub map: OsuMap,
    pub leaderboard: Vec<RoomLeaderboardItem>,
    pub scores: HashMap<u32, DailyChallengeScore, IntHasher>, /* TODO: slim down Score into
                                                               * smaller type */
    pub author: AuthorBuilder,
    pub footer: FooterBuilder,
    pub description: String,
    pub start_time: OffsetDateTime,
}

impl DailyChallengeDay {
    pub const FIRST_DATE: Date = date!(2024 - 09 - 30);

    pub async fn new(osu_id: Option<u32>, date: Date) -> Result<Self> {
        let room = Context::redis().daily_challenge(date).await?;

        let Some(playlist_item) = room.current_playlist_item.as_ref() else {
            bail!("Missing current playlist item for room {}", room.room_id);
        };

        let room_id = room.room_id.to_native();
        let playlist_item_id = playlist_item.playlist_item_id.to_native();
        let playlist_map = &playlist_item.map;
        let map_id = playlist_map.map_id.to_native();
        let checksum = playlist_map.checksum.as_deref();

        let leaderboard_fut = Context::osu().room_leaderboard(room_id);
        let scores_fut = Context::osu().playlist_scores(room_id, playlist_item_id);
        let map_fut = Context::osu_map().map(map_id, checksum);

        let (leaderboard, scores, map) = match tokio::join!(leaderboard_fut, scores_fut, map_fut) {
            (Ok(leaderboard), Ok(scores), Ok(map)) => (leaderboard.leaderboard, scores, map),
            (Err(err), ..) => return Err(Report::new(err).wrap_err("Failed to fetch leaderboard")),
            (_, Err(err), _) => return Err(Report::new(err).wrap_err("Failed to fetch scores")),
            (.., Err(err)) => return Err(Report::new(err).wrap_err("Failed to fetch map")),
        };

        let total_scores = scores.total;

        let required_mods: GameMods = playlist_item
            .required_mods
            .deserialize(Strategy::<_, BoxedError>::wrap(&mut ()))
            .unwrap();

        let scores: HashMap<_, _, IntHasher> = scores
            .scores
            .into_iter()
            .map(|score| {
                let user_id = score.user_id;

                let score = DailyChallengeScore {
                    mods: score.mods,
                    grade: score.grade,
                    score_id: score.id,
                    ended_at: score.ended_at,
                };

                (user_id, score)
            })
            .collect();

        let mut pp_calc = Context::pp(&map)
            .lazer(true)
            .mode(playlist_item.mode)
            .mods(required_mods.clone());

        let stars = if let Some(attrs) = pp_calc.difficulty().await {
            attrs.stars() as f32
        } else {
            0.0
        };

        let mut description = format!(
            "\n:musical_note: [Song preview](https://b.ppy.sh/preview/{mapset_id}.mp3) \
            :frame_photo: [Full background](https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg)",
            mapset_id = playlist_map.mapset_id.to_native(),
        );

        match playlist_item.mode {
            GameMode::Osu => {
                let _ = write!(
                    description,
                    " :clapper: [Map preview](https://preview.tryz.id.vn/?b={map_id})",
                    map_id = playlist_map.map_id
                );
            }
            GameMode::Mania | GameMode::Taiko => {
                let _ = write!(
                    description,
                    " :clapper: [Map preview](https://osu-preview.jmir.xyz/preview#{map_id})",
                    map_id = playlist_map.map_id
                );
            }
            // Waiting on a preview website that supports catch
            GameMode::Catch => {}
        }

        let _ = write!(
            description,
            "\n{}",
            MapInfo::new(&map, stars).mods(&required_mods)
        );

        if !required_mods.is_empty() {
            let _ = write!(
                description,
                "\n**Required mods: {}**",
                ModsFormatter::new(&required_mods, false)
            );
        }

        let author =
            AuthorBuilder::new(room.name.as_str()).icon_url(Emote::from(playlist_item.mode).url());

        let mut footer_text = format!("Total scores: {total_scores}");

        if let Some(osu_id) = osu_id
            && let Some(i) = leaderboard.iter().position(|item| item.user_id == osu_id)
        {
            let _ = write!(footer_text, " • Your position: #{}", i + 1);
        }

        let footer = FooterBuilder::new(footer_text);
        let start_time = room.starts_at.try_deserialize::<BoxedError>().unwrap();

        Ok(Self {
            map,
            leaderboard,
            scores,
            author,
            footer,
            description,
            start_time,
        })
    }
}

pub struct DailyChallengeScore {
    pub mods: GameMods,
    pub grade: Grade,
    pub score_id: u64,
    pub ended_at: OffsetDateTime,
}
