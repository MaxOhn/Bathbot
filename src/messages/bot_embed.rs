use crate::messages::{
    AuthorDescThumbData, AuthorDescThumbTitleData, CommonData, LeaderboardData, ProfileData,
    ScoreMultiData, ScoreSingleData, SimulateData,
};

use serenity::{builder::CreateEmbed, utils::Colour};

pub enum BotEmbed {
    UserScoreSingle(Box<ScoreSingleData>),
    UserScoreMulti(Box<ScoreMultiData>),
    AuthorDescThumb(AuthorDescThumbData),
    Profile(ProfileData),
    AuthorDescThumbTitle(AuthorDescThumbTitleData),
    SimulateScore(Box<SimulateData>),
    UserCommonScores(CommonData),
    Leaderboard(LeaderboardData),
}

impl BotEmbed {
    pub fn create(self, e: &mut CreateEmbed) -> &mut CreateEmbed {
        e.color(Colour::DARK_GREEN);
        match self {
            BotEmbed::UserScoreMulti(data) => create_user_score_multi(e, *data),
            BotEmbed::AuthorDescThumb(data) => create_author_desc_thumb(e, data),
            BotEmbed::Profile(data) => create_profile(e, data),
            BotEmbed::AuthorDescThumbTitle(data) => create_author_desc_title_thumb(e, data),
            BotEmbed::UserCommonScores(data) => create_common(e, data),
            BotEmbed::Leaderboard(data) => create_leaderboard(e, data),
            BotEmbed::UserScoreSingle(_) => panic!(
                "Don't use 'create' for UserScoreSingle, use 'create_full' or 'minimize' instead"
            ),
            BotEmbed::SimulateScore(_) => panic!(
                "Don't use 'create' for SimulateScore, use 'create_full' or 'minimize' instead"
            ),
        }
    }

    pub fn create_full<'s, 'e>(&'s self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        e.color(Colour::DARK_GREEN);
        match self {
            BotEmbed::UserScoreSingle(data) => create_user_score_single(e, &data),
            BotEmbed::SimulateScore(data) => create_simulation(e, &data),
            _ => e,
        }
    }

    pub fn minimize<'s, 'e>(&'s self, e: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        e.color(Colour::DARK_GREEN);
        match self {
            BotEmbed::UserScoreSingle(data) => create_user_score_single_mini(e, data),
            BotEmbed::SimulateScore(data) => create_simulation_mini(e, data),
            _ => e,
        }
    }
}

fn create_simulation_mini<'d, 'e>(
    embed: &'e mut CreateEmbed,
    data: &'d SimulateData,
) -> &'e mut CreateEmbed {
    let pp = if let Some(prev_pp) = &data.prev_pp {
        format!("{} → {}", prev_pp, data.pp)
    } else {
        data.pp.clone()
    };
    let combo = if let Some(prev_combo) = &data.prev_combo {
        format!("{} → {}", prev_combo, data.combo)
    } else {
        data.combo.clone()
    };
    let title = format!("{} [{}]", data.title, data.stars);
    let name = format!(
        "{} ({}) [ {} ]",
        data.grade_completion_mods, data.acc, combo
    );
    let mut value = format!("{} {}", pp, data.hits);
    if let Some(misses) = data.removed_misses {
        if misses > 0 {
            value.push_str(&format!(" (+{}miss)", misses));
        }
    }
    embed
        .field(name, value, false)
        .thumbnail(&data.thumbnail)
        .url(&data.title_url)
        .title(title)
}

fn create_user_score_single_mini<'d, 'e>(
    embed: &'e mut CreateEmbed,
    data: &'d ScoreSingleData,
) -> &'e mut CreateEmbed {
    let name = format!(
        "{}\t{}\t({})\t{}",
        data.grade_completion_mods, data.score, data.acc, data.ago
    );
    let value = format!("{} [ {} ] {}", data.pp, data.combo, data.hits);
    let title = format!("{} [{}]", data.title, data.stars);
    embed
        .field(name, value, false)
        .thumbnail(&data.thumbnail)
        .title(title)
        .url(&data.title_url)
        .author(|a| {
            a.icon_url(&data.author_icon)
                .url(&data.author_url)
                .name(&data.author_text)
        })
}

fn create_simulation<'d, 'e>(
    embed: &'e mut CreateEmbed,
    data: &'d SimulateData,
) -> &'e mut CreateEmbed {
    let pp = if let Some(prev_pp) = &data.prev_pp {
        format!("{} → {}", prev_pp, data.pp)
    } else {
        data.pp.to_owned()
    };
    let combo = if let Some(prev_combo) = &data.prev_combo {
        format!("{} → {}", prev_combo, data.combo)
    } else {
        data.combo.to_owned()
    };
    let hits = if let Some(prev_hits) = &data.prev_hits {
        format!("{} → {}", prev_hits, data.hits,)
    } else {
        data.hits.to_owned()
    };
    embed
        .title(&data.title)
        .url(&data.title_url)
        .thumbnail(&data.thumbnail)
        .footer(|f| f.icon_url(&data.footer_url).text(&data.footer_text))
        .fields(vec![
            ("Grade", &data.grade_completion_mods, true),
            ("Acc", &data.acc, true),
            ("Combo", &combo, true),
            ("PP", &pp, false),
            ("Hits", &hits, false),
            ("Map Info", &data.map_info, false),
        ])
}

fn create_user_score_single<'d, 'e>(
    embed: &'e mut CreateEmbed,
    data: &'d ScoreSingleData,
) -> &'e mut CreateEmbed {
    if data.description.is_some() {
        embed.description(&data.description.as_ref().unwrap());
    }
    embed
        .title(&data.title)
        .url(&data.title_url)
        .timestamp(data.timestamp.clone())
        .thumbnail(&data.thumbnail)
        .footer(|f| f.icon_url(&data.footer_url).text(&data.footer_text))
        .fields(vec![
            ("Grade", &data.grade_completion_mods, true),
            ("Score", &data.score, true),
            ("Acc", &data.acc, true),
            ("PP", &data.pp, true),
            ("Combo", &data.combo, true),
            ("Hits", &data.hits, true),
            ("Map Info", &data.map_info, false),
        ])
        .author(|a| {
            a.icon_url(&data.author_icon)
                .url(&data.author_url)
                .name(&data.author_text)
        })
}

fn create_user_score_multi(embed: &mut CreateEmbed, data: ScoreMultiData) -> &mut CreateEmbed {
    embed
        .footer(|f| f.icon_url(&data.footer_url).text(&data.footer_text))
        .author(|a| {
            a.icon_url(&data.author_icon)
                .url(&data.author_url)
                .name(&data.author_text)
        })
        .title(data.title)
        .thumbnail(data.thumbnail)
        .url(data.title_url);
    if data.fields.is_empty() {
        embed.description("No scores found")
    } else {
        embed.fields(data.fields)
    }
}

fn create_author_desc_thumb(
    embed: &mut CreateEmbed,
    data: AuthorDescThumbData,
) -> &mut CreateEmbed {
    embed
        .thumbnail(&data.thumbnail)
        .description(&data.description)
        .author(|a| {
            a.icon_url(data.author_icon)
                .url(data.author_url)
                .name(data.author_text)
        })
}

fn create_profile(embed: &mut CreateEmbed, data: ProfileData) -> &mut CreateEmbed {
    embed
        .footer(|f| f.text(&data.footer_text))
        .author(|a| {
            a.icon_url(&data.author_icon)
                .url(&data.author_url)
                .name(&data.author_text)
        })
        .thumbnail(data.thumbnail)
        .fields(data.fields)
}

fn create_common(embed: &mut CreateEmbed, data: CommonData) -> &mut CreateEmbed {
    embed
        .description(data.description)
        .thumbnail("attachment://avatar_fuse.png")
}

fn create_author_desc_title_thumb(
    embed: &mut CreateEmbed,
    data: AuthorDescThumbTitleData,
) -> &mut CreateEmbed {
    embed
        .thumbnail(&data.thumbnail)
        .description(&data.description)
        .title(&data.title)
        .author(|a| {
            a.icon_url(data.author_icon)
                .url(data.author_url)
                .name(data.author_text)
        })
}

fn create_leaderboard(embed: &mut CreateEmbed, data: LeaderboardData) -> &mut CreateEmbed {
    embed
        .footer(|f| f.icon_url(&data.footer_url).text(&data.footer_text))
        .author(|a| {
            a.icon_url(&data.author_icon)
                .url(&data.author_url)
                .name(&data.author_text)
        })
        .thumbnail(data.thumbnail)
        .description(data.description)
}
