use bathbot_macros::EmbedData;
use bathbot_util::{
    constants::OSU_BASE,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils,
};
use osu::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter};
use rosu_v2::prelude::{GameMode, Score};
use time::OffsetDateTime;
use twilight_model::channel::message::embed::EmbedField;

use crate::{
    core::Context,
    embeds::osu,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    util::osu::{grade_completion_mods, mode_emote},
};

#[derive(EmbedData)]
pub struct TrackNotificationEmbed {
    author: AuthorBuilder,
    description: String,
    fields: Vec<EmbedField>,
    timestamp: OffsetDateTime,
    title: String,
    thumbnail: String,
    url: String,
}

impl TrackNotificationEmbed {
    pub async fn new(
        user: &RedisData<User>,
        score: &Score,
        map: &OsuMap,
        idx: u8,
        ctx: &Context,
    ) -> Self {
        let description = format!("{} __**Personal Best #{idx}**__", mode_emote(score.mode));

        let attrs = ctx
            .pp(map)
            .mode(score.mode)
            .mods(score.mods)
            .performance()
            .await;

        let stars = attrs.stars();
        let max_pp = attrs.pp() as f32;
        let max_combo = attrs.max_combo() as u32;

        let title = if score.mode == GameMode::Mania {
            format!(
                "{} {} - {} [{}] [{stars:.2}★]",
                KeyFormatter::new(score.mods, map),
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
            )
        } else {
            format!(
                "{} - {} [{}] [{stars:.2}★]",
                map.artist().cow_escape_markdown(),
                map.title().cow_escape_markdown(),
                map.version().cow_escape_markdown(),
            )
        };

        let name = format!(
            "{}\t{score}\t({acc}%)",
            grade_completion_mods(score.mods, score.grade, score.total_hits(), map),
            score = WithComma::new(score.score),
            acc = round(score.accuracy)
        );

        let value = format!(
            "{pp} [ {combo} ] {hitresults}",
            pp = PpFormatter::new(score.pp, Some(max_pp)),
            combo = if score.mode == GameMode::Mania {
                let mut ratio = score.statistics.count_geki as f32;

                if score.statistics.count_300 > 0 {
                    ratio /= score.statistics.count_300 as f32
                }

                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                ComboFormatter::new(score.max_combo, Some(max_combo)).to_string()
            },
            hitresults = HitResultFormatter::new(score.mode, score.statistics.clone()),
        );

        Self {
            author: user.author_builder(),
            description,
            fields: fields![name, value, false],
            thumbnail: map.thumbnail().to_owned(),
            timestamp: score.ended_at,
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
        }
    }
}
