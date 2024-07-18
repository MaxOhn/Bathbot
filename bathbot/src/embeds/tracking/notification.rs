use bathbot_macros::EmbedData;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::{
    constants::OSU_BASE,
    fields,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder,
};
use osu::{ComboFormatter, HitResultFormatter, KeyFormatter, PpFormatter};
use rosu_v2::prelude::{GameMode, Grade, Score};
use time::OffsetDateTime;
use twilight_model::channel::message::embed::EmbedField;

use crate::{
    core::Context,
    embeds::osu,
    manager::{redis::RedisData, OsuMap},
    util::{osu::GradeCompletionFormatter, Emote},
};

#[derive(EmbedData)]
pub struct TrackNotificationEmbed {
    author: AuthorBuilder,
    description: String,
    fields: Vec<EmbedField>,
    footer: FooterBuilder,
    timestamp: OffsetDateTime,
    title: String,
    thumbnail: String,
    url: String,
}

impl TrackNotificationEmbed {
    pub async fn new(user: &RedisData<User>, score: &Score, map: &OsuMap, idx: u8) -> Self {
        let description = format!("__**Personal Best #{idx}**__");

        let attrs = Context::pp(map)
            .mode(score.mode)
            .mods(&score.mods)
            .performance()
            .await;

        let stars = attrs.stars();
        let max_combo = attrs.max_combo();

        let max_pp = score
            .pp
            .filter(|_| score.grade.eq_letter(Grade::X) && score.mode != GameMode::Mania)
            .unwrap_or(attrs.pp() as f32);

        let title = if score.mode == GameMode::Mania {
            format!(
                "{} {} - {} [{}] [{stars:.2}★]",
                KeyFormatter::new(&score.mods, map.attributes().build().cs as f32),
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
            // We don't use `GradeCompletionFormatter::new` so that it doesn't
            // use the score id to hyperlink the grade because those don't
            // work in embed field names.
            GradeCompletionFormatter::new_without_score(
                &score.mods,
                score.grade,
                score.total_hits(),
                map.mode(),
                map.n_objects()
            ),
            score = WithComma::new(score.score),
            acc = round(score.accuracy)
        );

        let value = format!(
            "{pp} [ {combo} ] {hitresults}",
            pp = PpFormatter::new(score.pp, Some(max_pp)),
            combo = if score.mode == GameMode::Mania {
                let mut ratio = score.statistics.perfect as f32;

                if score.statistics.great > 0 {
                    ratio /= score.statistics.great as f32
                }

                format!("**{}x** / {}", &score.max_combo, round(ratio))
            } else {
                ComboFormatter::new(score.max_combo, Some(max_combo)).to_string()
            },
            hitresults =
                HitResultFormatter::new(score.mode, score.statistics.as_legacy(score.mode)),
        );

        let footer = FooterBuilder::new(map.footer_text()).icon_url(Emote::from(score.mode).url());

        Self {
            author: user.author_builder(),
            description,
            fields: fields![name, value, false],
            footer,
            thumbnail: map.thumbnail().to_owned(),
            timestamp: score.ended_at,
            title,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
        }
    }
}
