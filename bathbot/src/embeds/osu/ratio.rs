use std::{collections::BTreeMap, fmt::Write};

use bathbot_macros::EmbedData;
use bathbot_model::rosu_v2::user::User;
use bathbot_util::AuthorBuilder;
use rosu_v2::{
    model::GameMode,
    prelude::{Grade, Score},
};

use crate::manager::redis::RedisData;

#[derive(EmbedData)]
pub struct RatioEmbed {
    description: String,
    thumbnail: String,
    author: AuthorBuilder,
}

impl RatioEmbed {
    pub fn new(user: &RedisData<User>, scores: Vec<Score>) -> Self {
        let accs = [0, 90, 95, 97, 99];
        let mut categories: BTreeMap<u8, RatioCategory> = BTreeMap::new();

        for &acc in accs.iter() {
            categories.insert(acc, RatioCategory::default());
        }

        categories.insert(100, RatioCategory::default());

        for score in scores {
            let acc = score.accuracy;

            for &curr in accs.iter() {
                if acc > curr as f32 {
                    categories.get_mut(&curr).unwrap().add_score(&score);
                }
            }

            if score.grade.eq_letter(Grade::X) {
                categories.get_mut(&100).unwrap().add_score(&score);
            }
        }

        let thumbnail = user.avatar_url().to_owned();
        let mut description = String::with_capacity(256);

        let _ = writeln!(
            description,
            "```\n \
        Acc: #Scores |  Ratio | % misses\n\
        --------------+--------+---------"
        );

        let mut all_scores = Vec::with_capacity(6);
        let mut all_ratios = Vec::with_capacity(6);
        let mut all_misses = Vec::with_capacity(6);

        for (acc, c) in categories.into_iter() {
            if c.scores > 0 {
                let scores = c.scores;
                let ratio = c.ratio();
                let misses = c.miss_percent();

                let _ = writeln!(
                    description,
                    "{}{acc:>2}%: {scores:>7} | {ratio:>6.3} | {misses:>7.3}%",
                    if acc < 100 { ">" } else { "" },
                );

                all_scores.push(scores as i8);
                all_ratios.push(ratio);
                all_misses.push(misses);
            }
        }

        description.push_str("```");

        Self {
            description,
            thumbnail,
            author: user.author_builder(),
        }
    }
}

#[derive(Default)]
struct RatioCategory {
    pub scores: u8,
    pub count_geki: u32,
    pub count_300: u32,
    pub count_miss: u32,
    pub count_objects: u32,
}

impl RatioCategory {
    fn add_score(&mut self, s: &Score) {
        let stats = s.statistics.as_legacy(GameMode::Mania);

        self.scores += 1;
        self.count_geki += stats.count_geki;
        self.count_300 += stats.count_300;
        self.count_miss += stats.count_miss;
        self.count_objects += stats.count_geki
            + stats.count_300
            + stats.count_katu
            + stats.count_100
            + stats.count_50
            + stats.count_miss;
    }

    fn ratio(&self) -> f32 {
        if self.count_300 == 0 {
            self.count_geki as f32
        } else {
            self.count_geki as f32 / self.count_300 as f32
        }
    }

    fn miss_percent(&self) -> f32 {
        (self.count_objects > 0) as u8 as f32 * 100.0 * self.count_miss as f32
            / self.count_objects as f32
    }
}
