use crate::{
    embeds::{osu, Author, EmbedData},
    util::constants::AVATAR_URL,
    BotResult, Context,
};

use rosu::models::{GameMode, Grade, Score, User};
use std::{collections::BTreeMap, fmt::Write};
use twilight_embed_builder::image_source::ImageSource;

#[derive(Clone)]
pub struct RatioEmbed {
    description: String,
    thumbnail: ImageSource,
    author: Author,
}

impl RatioEmbed {
    pub async fn new(ctx: &Context, user: User, scores: Vec<Score>) -> BotResult<Self> {
        let mut accs = vec![0, 90, 95, 97, 99];
        let mut categories: BTreeMap<u8, RatioCategory> = BTreeMap::new();
        for &acc in accs.iter() {
            categories.insert(acc, RatioCategory::default());
        }
        categories.insert(100, RatioCategory::default());
        for score in scores {
            let acc = score.accuracy(GameMode::MNA);
            for &curr in accs.iter() {
                if acc > curr as f32 {
                    categories.get_mut(&curr).unwrap().add_score(&score);
                }
            }
            if score.grade.eq_letter(Grade::X) {
                categories.get_mut(&100).unwrap().add_score(&score);
            }
        }
        let thumbnail = ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap();
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
                    "{}{:>2}%: {:>7} | {:>6.3} | {:>7.3}%",
                    if acc < 100 { ">" } else { "" },
                    acc,
                    scores,
                    ratio,
                    misses,
                );
                all_scores.push(scores as i8);
                all_ratios.push(ratio);
                all_misses.push(misses);
            }
        }
        let previous_ratios = ctx
            .psql()
            .update_ratios(&user.username, &all_scores, &all_ratios, &all_misses)
            .await?;
        if let Some(ratios) = previous_ratios {
            if ratios.scores != all_scores
                || ratios.ratios != all_ratios
                || ratios.misses != all_misses
            {
                let _ = writeln!(description, "--------------+--------+---------");
                accs.push(100);
                for (i, acc) in accs.iter().enumerate() {
                    let any_changes = match (ratios.scores.get(i), all_scores.get(i)) {
                        (Some(new), Some(old)) => new != old,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    } || match (ratios.ratios.get(i), all_ratios.get(i)) {
                        (Some(new), Some(old)) => (new - old).abs() >= 0.0005,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    } || match (ratios.misses.get(i), all_misses.get(i)) {
                        (Some(new), Some(old)) => (new - old).abs() >= 0.0005,
                        (None, Some(_)) => true,
                        (Some(_), None) => true,
                        (None, None) => false,
                    };
                    if any_changes {
                        let _ = writeln!(
                            description,
                            "{}{:>2}%: {:>+7} | {:>+6.3} | {:>+7.3}%",
                            if *acc < 100 { ">" } else { "" },
                            acc,
                            *all_scores.get(i).unwrap_or_else(|| &0)
                                - *ratios.scores.get(i).unwrap_or_else(|| &0),
                            *all_ratios.get(i).unwrap_or_else(|| &0.0)
                                - *ratios.ratios.get(i).unwrap_or_else(|| &0.0),
                            *all_misses.get(i).unwrap_or_else(|| &0.0)
                                - *ratios.misses.get(i).unwrap_or_else(|| &0.0),
                        );
                    }
                }
            }
        }
        description.push_str("```");
        Ok(Self {
            description,
            thumbnail,
            author: osu::get_user_author(&user),
        })
    }
}

impl EmbedData for RatioEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
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
        self.scores += 1;
        self.count_geki += s.count_geki;
        self.count_300 += s.count300;
        self.count_miss += s.count_miss;
        self.count_objects +=
            s.count_geki + s.count300 + s.count_katu + s.count100 + s.count50 + s.count_miss;
    }

    fn ratio(&self) -> f32 {
        if self.count_300 == 0 {
            self.count_geki as f32
        } else {
            self.count_geki as f32 / self.count_300 as f32
        }
    }

    fn miss_percent(&self) -> f32 {
        if self.count_objects > 0 {
            100.0 * self.count_miss as f32 / self.count_objects as f32
        } else {
            0.0
        }
    }
}
