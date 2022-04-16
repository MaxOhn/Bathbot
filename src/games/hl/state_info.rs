use std::borrow::Cow;

use rosu_v2::prelude::{Beatmap, Beatmapset, GameMods, Grade, Score, UserCompact};

use crate::{
    embeds::get_mods,
    util::{
        numbers::{round, with_comma_int},
        osu::grade_emote,
        Emote,
    },
};

pub struct GameStateInfo {
    user_id: u32,
    pub avatar: String,
    map_id: u32,
    pub player_string: String,
    map_string: String,
    mods: GameMods,
    pub pp: f32,
    combo: u32,
    max_combo: u32,
    score: u32,
    acc: f32,
    miss_count: u32,
    grade: Grade,
    pub cover: String,
}

impl GameStateInfo {
    pub fn new(user: UserCompact, map: Beatmap, score: Score) -> Self {
        let Beatmapset {
            mapset_id,
            artist,
            title,
            ..
        } = map.mapset.unwrap();

        let rank = user
            .statistics
            .as_ref()
            .and_then(|stats| stats.global_rank)
            .unwrap_or(0);

        let country_code = user.country_code.to_lowercase();

        Self {
            user_id: user.user_id,
            avatar: user.avatar_url,
            map_id: map.map_id,
            player_string: format!(":flag_{country_code}: {} (#{rank})", user.username,),
            map_string: format!("[{artist} - {title} [{}]]({})", map.version, map.url),
            mods: score.mods,
            pp: round(score.pp.unwrap_or(0.0)),
            combo: score.max_combo,
            max_combo: map.max_combo.unwrap_or(0),
            score: score.score,
            acc: round(score.accuracy),
            miss_count: score.statistics.count_miss,
            grade: score.grade,
            cover: format!("https://assets.ppy.sh/beatmaps/{mapset_id}/covers/cover.jpg",),
        }
    }

    pub fn play_string(&self, pp_visible: bool) -> String {
        format!(
            "**{} {}**\n{} {} • **{}%** • **{}x**/{}x {}• **{}pp**",
            self.map_string,
            get_mods(self.mods),
            grade_emote(self.grade),
            with_comma_int(self.score),
            self.acc,
            self.combo,
            self.max_combo,
            if self.miss_count > 0 {
                format!("• **{}{}** ", self.miss_count, Emote::Miss.text())
            } else {
                String::new()
            },
            if pp_visible {
                self.pp.to_string().into()
            } else {
                Cow::Borrowed("???")
            }
        )
    }
}

impl PartialEq for GameStateInfo {
    fn eq(&self, other: &Self) -> bool {
        self.user_id == other.user_id && self.map_id == other.map_id
    }
}
