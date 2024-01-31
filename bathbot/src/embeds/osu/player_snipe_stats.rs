use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::{rosu_v2::user::User, SnipePlayer};
use bathbot_util::{
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    fields,
    numbers::{round, WithComma},
    AuthorBuilder, CowUtils, FooterBuilder,
};
use osu::{ComboFormatter, HitResultFormatter, PpFormatter};
use rosu_v2::prelude::{GameMode, Score};
use twilight_model::channel::message::embed::EmbedField;

use crate::{
    core::Context,
    embeds::{attachment, osu},
    manager::{redis::RedisData, OsuMap},
    util::osu::grade_completion_mods,
};

#[derive(EmbedData)]
pub struct PlayerSnipeStatsEmbed {
    description: String,
    thumbnail: String,
    title: &'static str,
    url: String,
    author: AuthorBuilder,
    footer: FooterBuilder,
    image: String,
    fields: Vec<EmbedField>,
}

impl PlayerSnipeStatsEmbed {
    pub async fn new(
        user: &RedisData<User>,
        player: SnipePlayer,
        oldest_score: &Score,
        oldest_map: &OsuMap,
        ctx: &Context,
    ) -> Self {
        let footer_text = format!(
            "{:+} #1{} since last update",
            player.difference,
            if player.difference == 1 { "" } else { "s" }
        );

        let (description, fields) = if player.count_first == 0 {
            ("No national #1s :(".to_owned(), Vec::new())
        } else {
            let mut fields = Vec::with_capacity(7);
            let mut description = String::with_capacity(256);

            let _ = writeln!(
                description,
                "**Total #1s: {}** | ranked: {} | loved: {}",
                player.count_first, player.count_ranked, player.count_loved
            );

            fields![fields {
                "Average PP:", WithComma::new(player.avg_pp).to_string(), true;
                "Average acc:", format!("{:.2}%", player.avg_acc), true;
                "Average stars:", format!("{:.2}★", player.avg_stars), true;
            }];

            let mut calc = ctx.pp(oldest_map).mods(oldest_score.mods.bits());

            let attrs = calc.performance().await;
            let stars = attrs.stars() as f32;
            let max_pp = attrs.pp() as f32;
            let max_combo = attrs.max_combo() as u32;

            let pp = match oldest_score.pp {
                Some(pp) => pp,
                None => calc.score(oldest_score).performance().await.pp() as f32,
            };

            // TODO: update formatting
            let value = format!(
                "**[{map}]({OSU_BASE}b/{id})**\t\
                {grade}\t[{stars:.2}★]\t{score}\t({acc}%)\t[{combo}]\t\
                [{pp}]\t {hits}\t{ago}",
                map = player.oldest_first.map.cow_escape_markdown(),
                id = oldest_map.map_id(),
                grade = grade_completion_mods(
                    &oldest_score.mods,
                    oldest_score.grade,
                    oldest_score.total_hits(),
                    oldest_map.mode(),
                    oldest_map.n_objects() as u32,
                ),
                score = WithComma::new(oldest_score.score),
                acc = round(oldest_score.accuracy),
                combo = ComboFormatter::new(oldest_score.max_combo, Some(max_combo)),
                pp = PpFormatter::new(Some(pp), Some(max_pp)),
                hits = HitResultFormatter::new(
                    GameMode::Osu,
                    oldest_score.statistics.as_legacy(GameMode::Osu)
                ),
                ago = HowLongAgoDynamic::new(&oldest_score.ended_at)
            );

            fields![fields { "Oldest national #1:", value, false }];

            let mut count_mods = player.count_mods.unwrap();
            let mut value = String::with_capacity(count_mods.len() * 7);
            count_mods.sort_unstable_by(|(_, c1), (_, c2)| c2.cmp(c1));
            let mut iter = count_mods.into_iter();

            if let Some((first_mods, first_amount)) = iter.next() {
                let _ = write!(value, "`{first_mods}: {first_amount}`");
                let mut idx = 0;

                for (mods, amount) in iter {
                    match idx {
                        2 => {
                            idx = 0;
                            let _ = write!(value, " >\n`{mods}: {amount}`");
                        }
                        _ => {
                            idx += 1;
                            let _ = write!(value, " > `{mods}: {amount}`");
                        }
                    }
                }
            }

            fields![fields { "Most used mods:", value, false }];

            (description, fields)
        };

        let (user_id, country_code, avatar_url) = match user {
            RedisData::Original(user) => {
                let user_id = user.user_id;
                let country_code = user.country_code.as_str();
                let avatar_url = user.avatar_url.as_ref();

                (user_id, country_code, avatar_url)
            }
            RedisData::Archive(user) => {
                let user_id = user.user_id;
                let country_code = user.country_code.as_str();
                let avatar_url = user.avatar_url.as_ref();

                (user_id, country_code, avatar_url)
            }
        };

        let url = format!(
            "https://snipe.huismetbenen.nl/player/{code}/osu/{user_id}",
            code = country_code.to_lowercase(),
        );

        Self {
            url,
            fields,
            description,
            footer: FooterBuilder::new(footer_text),
            author: user.author_builder(),
            title: "National #1 statistics",
            image: attachment("stats_graph.png"),
            thumbnail: avatar_url.to_owned(),
        }
    }
}
