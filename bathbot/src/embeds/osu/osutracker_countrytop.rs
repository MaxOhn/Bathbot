use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use bathbot_model::OsuTrackerCountryScore;
use bathbot_util::{
    constants::OSU_BASE,
    numbers::{round, WithComma},
    osu::flag_url,
    CowUtils, FooterBuilder,
};

use crate::{
    commands::osu::{OsuTrackerCountryDetailsCompact, ScoreOrder},
    pagination::Pages,
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
        pages: &Pages,
    ) -> Self {
        let national = !details.code.is_empty();

        let url = format!(
            "https://osutracker.com/country/{code}",
            code = national.then(|| details.code.as_str()).unwrap_or("Global")
        );

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer_text =
            format!("Page {page}/{pages} • Data originates from https://osutracker.com");
        let footer = FooterBuilder::new(footer_text);

        let title = format!("Total PP: {}pp", WithComma::new(details.pp));

        let mut description = String::with_capacity(scores.len() * 160);

        for (score, i) in scores.iter() {
            let _ = writeln!(
                description,
                "**{i}.** [{map_name}]({OSU_BASE}b/{map_id}) **+{mods}**\n\
                > by __[{user}]({OSU_BASE}u/{adjusted_user})__ • **{pp}pp** • {acc}% • <t:{timestamp}:R>{appendix}",
                map_name = score.name.cow_escape_markdown(),
                map_id = score.map_id,
                mods = score.mods,
                user = score.player.cow_escape_markdown(),
                adjusted_user = score.player.cow_replace(' ', "%20"),
                pp = round(score.pp),
                acc = round(score.acc),
                timestamp = score.ended_at.unix_timestamp(),
                appendix = OrderAppendix::new(sort_by, score),
            );
        }

        let thumbnail = national
            .then(|| flag_url(details.code.as_str()))
            .unwrap_or_default();

        Self {
            description,
            footer,
            thumbnail,
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

impl Display for OrderAppendix<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.sort_by {
            ScoreOrder::Acc | ScoreOrder::Date | ScoreOrder::Pp => Ok(()),
            ScoreOrder::Length => {
                let clock_rate = self.score.mods.legacy_clock_rate();
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
