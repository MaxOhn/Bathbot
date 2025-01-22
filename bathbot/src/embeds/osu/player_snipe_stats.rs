use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_model::SnipePlayer;
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
    manager::{redis::osu::CachedUser, OsuMap},
    util::{osu::GradeCompletionFormatter, CachedUserExt},
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
        user: &CachedUser,
        player: SnipePlayer,
        oldest: Option<&(Score, OsuMap)>,
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

            if let Some((oldest_score, oldest_map)) = oldest {
                let mut calc = Context::pp(oldest_map).mods(oldest_score.mods.clone());

                let attrs = calc.performance().await;
                let stars = attrs.stars() as f32;
                let max_pp = attrs.pp() as f32;
                let max_combo = attrs.max_combo();

                let pp = match oldest_score.pp {
                    Some(pp) => pp,
                    None => calc.score(oldest_score).performance().await.pp() as f32,
                };

                // TODO: update formatting
                let value = format!(
                    "**[{artist} - {title} [{version}]]({OSU_BASE}b/{id})**\t\
                    {grade}\t[{stars:.2}★]\t{score}\t({acc}%)\t[{combo}]\t\
                    [{pp}]\t {hits}\t{ago}",
                    artist = oldest_map.artist().cow_escape_markdown(),
                    title = oldest_map.title().cow_escape_markdown(),
                    version = oldest_map.version().cow_escape_markdown(),
                    id = oldest_map.map_id(),
                    grade = GradeCompletionFormatter::new(
                        oldest_score,
                        oldest_map.mode(),
                        oldest_map.n_objects(),
                    ),
                    score = WithComma::new(oldest_score.score),
                    acc = round(oldest_score.accuracy),
                    combo = ComboFormatter::new(oldest_score.max_combo, Some(max_combo)),
                    pp = PpFormatter::new(Some(pp), Some(max_pp)),
                    hits = HitResultFormatter::new(GameMode::Osu, &oldest_score.statistics),
                    ago = HowLongAgoDynamic::new(&oldest_score.ended_at)
                );

                fields![fields { "Oldest national #1:", value, false }];
            }

            let mut count_mods = player.count_mods;
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

        let user_id = user.user_id.to_native();
        let country_code = user.country_code.as_str();
        let avatar_url = user.avatar_url.as_ref();

        let url = match oldest.map_or(GameMode::Osu, |(score, _)| score.mode) {
            GameMode::Osu => format!(
                "https://snipe.huismetbenen.nl/player/{code}/osu/{user_id}",
                code = country_code.to_lowercase(),
            ),
            GameMode::Catch => format!("https://snipes.kittenroleplay.com/player/{user_id}/catch"),
            GameMode::Mania => format!("https://snipes.kittenroleplay.com/player/{user_id}/mania"),
            GameMode::Taiko => unimplemented!(),
        };

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
