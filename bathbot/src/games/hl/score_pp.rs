use std::{cmp::Ordering, fmt::Display};

use bathbot_model::rosu_v2::ranking::ArchivedRankingsUser;
use bathbot_util::{
    constants::OSU_BASE,
    numbers::{round, WithComma},
};
use eyre::{Report, Result, WrapErr};
use image::{GenericImageView, ImageBuffer};
use rand::Rng;
use rosu_v2::prelude::{
    CountryCode, GameMode, GameMods, Grade, Score, UserCompact as UserCompactRosu, Username,
};

use crate::{
    core::Context,
    embeds::ModsFormatter,
    games::hl::mapset_cover,
    manager::{redis::RedisData, OsuMapSlim},
    util::{osu::grade_emote, Emote},
};

use super::{kind::GameStateKind, H, W};

const ALPHA_THRESHOLD: u8 = 20;

pub(super) struct ScorePp {
    user_id: u32,
    pub avatar_url: Box<str>,
    map_id: u32,
    pub mapset_id: u32,
    pub player_string: String,
    map_string: String,
    mods: GameMods,
    pub pp: f32,
    combo: u32,
    max_combo: Option<u32>,
    score: u32,
    acc: f32,
    miss_count: u32,
    grade: Grade,
}

impl ScorePp {
    pub async fn random(
        ctx: &Context,
        mode: GameMode,
        prev_pp: f32,
        curr_score: u32,
    ) -> Result<Self> {
        let max_play = 25 - curr_score.min(24);
        let min_play = 24 - 2 * curr_score.min(12);
        let max_rank = 5000 - (mode != GameMode::Osu) as u32 * 1000;

        let (rank, play): (u32, u32) = {
            let mut rng = rand::thread_rng();

            (
                rng.gen_range(1..=max_rank),
                rng.gen_range(min_play..max_play),
            )
        };

        let page = ((rank - 1) / 50) + 1;
        let idx = ((rank - 1) % 50) as usize;

        let ranking = ctx
            .redis()
            .pp_ranking(mode, page, None)
            .await
            .wrap_err("failed to get cached pp ranking")?;

        let player = match ranking {
            RedisData::Original(mut ranking) => UserCompact::from(ranking.ranking.swap_remove(idx)),
            RedisData::Archive(ranking) => UserCompact::from(&ranking.ranking[idx]),
        };

        let mut plays = ctx
            .osu()
            .user_scores(player.user_id)
            .limit(100)
            .mode(mode)
            .best()
            .await
            .wrap_err("Failed to get user scores")?;

        plays.sort_unstable_by(|a, b| {
            let a_pp = (a.pp.unwrap_or(0.0) - prev_pp).abs();
            let b_pp = (b.pp.unwrap_or(0.0) - prev_pp).abs();

            a_pp.partial_cmp(&b_pp).unwrap_or(Ordering::Equal)
        });

        let play = plays.swap_remove(play as usize);

        let map_fut = ctx.osu_map().map_slim(play.map_id);
        let attrs_fut = ctx.osu_map().difficulty(play.map_id, play.mode, play.mods);

        let (map_res, attrs_res) = tokio::join!(map_fut, attrs_fut);

        let map = map_res.wrap_err("Failed to get beatmap")?;

        let max_combo = match attrs_res {
            Ok(attrs) => Some(attrs.max_combo() as u32),
            Err(err) => {
                let wrap = "Failed to get difficulty attributes";
                warn!("{:?}", Report::new(err).wrap_err(wrap));

                None
            }
        };

        Ok(Self::new(player, map, max_combo, play))
    }

    pub async fn image(
        ctx: &Context,
        pfp1: &str,
        pfp2: &str,
        mapset1: u32,
        mapset2: u32,
    ) -> Result<String> {
        let cover1 = mapset_cover(mapset1);
        let cover2 = mapset_cover(mapset2);

        // Gather the profile pictures and map covers
        let client = ctx.client();

        let (pfp_left, pfp_right, bg_left, bg_right) = tokio::try_join!(
            client.get_avatar(pfp1),
            client.get_avatar(pfp2),
            client.get_mapset_cover(&cover1),
            client.get_mapset_cover(&cover2),
        )
        .wrap_err("failed to retrieve some image")?;

        let pfp_left = image::load_from_memory(&pfp_left)
            .wrap_err("failed to load pfp1 from memory")?
            .thumbnail(128, 128);

        let pfp_right = image::load_from_memory(&pfp_right)
            .wrap_err("failed to load pfp2 from memory")?
            .thumbnail(128, 128);

        let bg_left =
            image::load_from_memory(&bg_left).wrap_err("failed to load left bg from memory")?;

        let bg_right =
            image::load_from_memory(&bg_right).wrap_err("failed to load right bg from memory")?;

        // Combine the images
        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        for (x, y, pixel) in pfp_left.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(x, y, pixel);
            }
        }

        let pfp_right_width = pfp_right.width();

        for (x, y, pixel) in pfp_right.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(W - pfp_right_width + x, y, pixel);
            }
        }

        const ID_START_IDX: usize = 17; // "https://a.ppy.sh/{user_id}?{hash}.png"

        let content = format!(
            "{user1} ({mapset1}) ~ {user2} ({mapset2})",
            user1 = pfp1
                .find('?')
                .and_then(|idx| pfp1.get(ID_START_IDX..idx))
                .unwrap_or(pfp1),
            user2 = pfp2
                .find('?')
                .and_then(|idx| pfp2.get(ID_START_IDX..idx))
                .unwrap_or(pfp2),
        );

        GameStateKind::upload_image(ctx, blipped.as_raw(), content).await
    }

    pub fn play_string(&self, pp_visible: bool) -> String {
        format!(
            "**{map} {mods}**\n{grade} {score} • **{acc}%** • **{combo}x**{max_combo} {miss}• **{pp}pp**",
            map = self.map_string,
            mods = ModsFormatter::new(self.mods),
            grade = grade_emote(self.grade),
            score = WithComma::new(self.score),
            acc = self.acc,
            combo = self.combo,
            max_combo = match self.max_combo {
                Some(ref combo) => format!("/{combo}x"),
                None => String::new(),
            },
            miss = if self.miss_count > 0 {
                format!("• **{}{}** ", self.miss_count, Emote::Miss.text())
            } else {
                String::new()
            },
            pp = if pp_visible {
                &self.pp as &dyn Display
            } else {
                &"???" as &dyn Display
            }
        )
    }

    fn new(user: UserCompact, map: OsuMapSlim, max_combo: Option<u32>, score: Score) -> Self {
        let UserCompact {
            avatar_url,
            country_code,
            global_rank,
            user_id,
            username,
        } = user;

        let country_code = country_code.to_lowercase();

        Self {
            user_id,
            avatar_url,
            map_id: map.map_id(),
            mapset_id: map.mapset_id(),
            player_string: format!(":flag_{country_code}: {username} (#{global_rank})"),
            map_string: format!(
                "[{artist} - {title} [{version}]]({OSU_BASE}b/{map_id})",
                artist = map.artist(),
                title = map.title(),
                version = map.version(),
                map_id = map.map_id(),
            ),
            mods: score.mods,
            pp: round(score.pp.unwrap_or(0.0)),
            combo: score.max_combo,
            max_combo,
            score: score.score,
            acc: round(score.accuracy),
            miss_count: score.statistics.count_miss,
            grade: score.grade,
        }
    }
}

impl PartialEq for ScorePp {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.user_id == other.user_id && self.map_id == other.map_id
    }
}

struct UserCompact {
    avatar_url: Box<str>,
    country_code: CountryCode,
    global_rank: u32,
    user_id: u32,
    username: Username,
}

impl From<UserCompactRosu> for UserCompact {
    #[inline]
    fn from(user: UserCompactRosu) -> Self {
        Self {
            avatar_url: user.avatar_url.into_boxed_str(),
            country_code: user.country_code,
            global_rank: user
                .statistics
                .and_then(|stats| stats.global_rank)
                .unwrap_or(0),
            user_id: user.user_id,
            username: user.username,
        }
    }
}

impl From<&ArchivedRankingsUser> for UserCompact {
    #[inline]
    fn from(user: &ArchivedRankingsUser) -> Self {
        Self {
            avatar_url: user.avatar_url.as_ref().into(),
            country_code: user.country_code.as_str().into(),
            global_rank: user
                .statistics
                .as_ref()
                .map_or(0, |stats| stats.global_rank),
            user_id: user.user_id,
            username: user.username.as_str().into(),
        }
    }
}
