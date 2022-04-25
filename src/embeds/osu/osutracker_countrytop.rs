use std::fmt::{self, Write};

use command_macros::EmbedData;
use rosu_v2::prelude::GameMods;

use crate::{
    commands::osu::{OsuTrackerCountryDetailsCompact, ScoreOrder},
    custom_client::OsuTrackerCountryScore,
    util::{
        builder::FooterBuilder,
        constants::OSU_BASE,
        numbers::{round, with_comma_float},
        osu::flag_url,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct OsuTrackerCountryTopEmbed {
    description: String,
    footer: FooterBuilder,
    thumbnail: String,
    title: String,
    url: String,
}

impl OsuTrackerCountryTopEmbed {
    pub fn new(
        details: &OsuTrackerCountryDetailsCompact,
        scores: &[(OsuTrackerCountryScore, usize)],
        sort_by: ScoreOrder,
        (page, pages): (usize, usize),
    ) -> Self {
        let url = format!("https://osutracker.com/country/{}", details.code);

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

        let title = format!("Total PP: {}pp", with_comma_float(details.pp));

        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter() {
            let _ = writeln!(
                description,
                "**{i}.** [{map_name}]({OSU_BASE}b/{map_id}) **+{mods}**\n\
                > by __[{user}]({OSU_BASE}u/{adjusted_user})__ • **{pp}pp** • {acc}% • <t:{timestamp}:R>{appendix}",
                map_name = score.name,
                map_id = score.map_id,
                mods = score.mods,
                user = score.player.cow_replace('_', "\\_"),
                adjusted_user = score.player.cow_replace(' ', "%20"),
                pp = round(score.pp),
                acc = round(score.acc),
                timestamp = score.created_at.timestamp(),
                appendix = OrderAppendix::new(sort_by, score),
            );
        }

        Self {
            description,
            footer,
            thumbnail: flag_url(details.code.as_str()),
            title,
            url,
        }
    }
}

struct OrderAppendix<'s> {
    sort_by: ScoreOrder,
    score: &'s OsuTrackerCountryScore,
}

impl<'s> OrderAppendix<'s> {
    pub fn new(sort_by: ScoreOrder, score: &'s OsuTrackerCountryScore) -> Self {
        Self { sort_by, score }
    }
}

impl fmt::Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.sort_by {
            ScoreOrder::Acc | ScoreOrder::Date | ScoreOrder::Pp => Ok(()),
            ScoreOrder::Length => {
                let mods = self.score.mods;

                let clock_rate = if mods.contains(GameMods::DoubleTime) {
                    1.5
                } else if mods.contains(GameMods::HalfTime) {
                    0.75
                } else {
                    1.0
                };

                let secs = (self.score.seconds_total as f32 / clock_rate) as u32;

                write!(f, " • `{}:{:0>2}`", secs / 60, secs % 60)
            }
            ScoreOrder::Misses => write!(
                f,
                " • {}miss{plural}",
                self.score.n_misses,
                plural = if self.score.n_misses != 1 { "es" } else { "" }
            ),
            _ => unreachable!(),
        }
    }
}
