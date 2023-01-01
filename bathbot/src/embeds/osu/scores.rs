use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::EmbedData;
use rosu_v2::prelude::{GameMode, Score};

use crate::{
    commands::osu::CompareEntry,
    core::BotConfig,
    manager::{
        redis::{osu::User, RedisData},
        OsuMap,
    },
    pagination::Pages,
    util::{
        builder::{AuthorBuilder, FooterBuilder},
        constants::{AVATAR_URL, MAP_THUMB_URL, OSU_BASE},
        datetime::HowLongAgoDynamic,
        numbers::{round, WithComma},
        osu::ScoreSlim,
        CowUtils, Emote, ScoreExt,
    },
};

use super::HitResultFormatter;

#[derive(EmbedData)]
pub struct ScoresEmbed {
    description: String,
    thumbnail: String,
    footer: FooterBuilder,
    author: AuthorBuilder,
    title: String,
    url: String,
}

impl ScoresEmbed {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user: &RedisData<User>,
        map: &OsuMap,
        entries: &[CompareEntry],
        pinned: &[Score],
        personal: &[Score],
        global: Option<(usize, usize)>,
        pp_idx: usize,
        pages: &Pages,
    ) -> Self {
        let page = pages.curr_page();
        let pages = pages.last_page();

        let mut description = String::with_capacity(512);
        let pp_idx = (page == pp_idx / 10 + 1).then_some(pp_idx % 10);
        let mut args = WriteArgs::new(&mut description, pinned, personal, global, pp_idx);

        let max_combo_ = map.max_combo().unwrap_or(0);
        let mut entries = entries.iter();

        if page == 1 {
            if let Some(entry) = entries.next() {
                let personal = personal_idx(entry, args.personal);

                if personal.is_some() || matches!(args.global, Some((0, _))) {
                    args.description.push_str("__**");

                    if let Some(idx) = personal {
                        let _ = write!(args.description, "Personal Best #{idx}");
                    }

                    if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == 0) {
                        if personal.is_some() {
                            args.description.push_str(" and ");
                        }

                        let _ = write!(args.description, "Global Top #{idx}");
                    }

                    args.description.push_str("**__");
                }

                let mut pinned = args.pinned.iter();

                if pinned.any(|s| s.score_id == entry.score.score_id && s.mods == entry.score.mods)
                {
                    args.description.push_str(" 📌");
                }

                if !args.description.is_empty() {
                    args.description.push('\n');
                }

                let _ = writeln!(
                    args.description,
                    "{grade} **+{mods}** [{stars:.2}★] • {score} • {acc}%\n\
                    {pp_format}**{pp}**{pp_format}/{max_pp}PP • **{combo}x**/{max_combo}x\n\
                    {hits} {timestamp}",
                    grade = BotConfig::get().grade(entry.score.grade),
                    mods = entry.score.mods,
                    stars = entry.stars,
                    score = WithComma::new(entry.score.score),
                    acc = round(entry.score.accuracy),
                    pp_format = if pp_idx == Some(0) { "" } else { "~~" },
                    pp = round(entry.score.pp),
                    max_pp = round(entry.score.pp.max(entry.max_pp)),
                    combo = entry.score.max_combo,
                    max_combo = OptionFormat::new(map.max_combo()),
                    hits =
                        HitResultFormatter::new(entry.score.mode, entry.score.statistics.clone()),
                    timestamp = HowLongAgoDynamic::new(&entry.score.ended_at)
                );

                if let Some(entry) = entries.next() {
                    args.description
                        .push_str("\n__Other scores on the beatmap:__\n");
                    write_compact_entry(&mut args, 1, entry, max_combo_);
                }
            }
        }

        for (entry, i) in entries.zip(2..) {
            write_compact_entry(&mut args, i, entry, max_combo_);
        }

        if args.description.is_empty() {
            args.description.push_str("No scores found");
        }

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} map",
            status = map.status(),
        );

        let footer_icon = format!("{AVATAR_URL}{creator_id}", creator_id = map.creator_id());
        let footer = FooterBuilder::new(footer_text).icon_url(footer_icon);

        let mut title_text = String::with_capacity(32);

        let _ = write!(
            title_text,
            "{artist} - {title} [{version}]",
            artist = map.artist().cow_escape_markdown(),
            title = map.title().cow_escape_markdown(),
            version = map.version().cow_escape_markdown()
        );

        if map.mode() == GameMode::Mania {
            let _ = write!(title_text, "[{}K] ", map.cs() as u32);
        }

        Self {
            description,
            footer,
            thumbnail: format!("{MAP_THUMB_URL}{}l.jpg", map.mapset_id()),
            title: title_text,
            url: format!("{OSU_BASE}b/{}", map.map_id()),
            author: user.author_builder(),
        }
    }
}

struct WriteArgs<'c> {
    description: &'c mut String,
    pinned: &'c [Score],
    personal: &'c [Score],
    global: Option<(usize, usize)>,
    pp_idx: Option<usize>,
}

impl<'c> WriteArgs<'c> {
    fn new(
        description: &'c mut String,
        pinned: &'c [Score],
        personal: &'c [Score],
        global: Option<(usize, usize)>,
        pp_idx: Option<usize>,
    ) -> Self {
        Self {
            description,
            pinned,
            personal,
            global,
            pp_idx,
        }
    }
}

fn personal_idx(entry: &CompareEntry, scores: &[Score]) -> Option<usize> {
    scores
        .iter()
        .position(|s| {
            (s.ended_at.unix_timestamp() - entry.score.ended_at.unix_timestamp()).abs() <= 2
        })
        .map(|i| i + 1)
}

fn write_compact_entry(args: &mut WriteArgs<'_>, i: usize, entry: &CompareEntry, max_combo: u32) {
    let config = BotConfig::get();

    let _ = write!(
        args.description,
        "{grade} **+{mods}** [{stars:.2}★] {pp_format}{pp:.2}pp{pp_format} \
        ({acc}%) {combo}x • {miss} {timestamp}",
        grade = config.grade(entry.score.grade),
        mods = entry.score.mods,
        stars = entry.stars,
        pp_format = if args.pp_idx == Some(i) { "**" } else { "~~" },
        pp = entry.score.pp,
        acc = round(entry.score.accuracy),
        combo = entry.score.max_combo,
        miss = MissFormat::new(&entry.score, max_combo),
        timestamp = HowLongAgoDynamic::new(&entry.score.ended_at),
    );

    let mut pinned = args.pinned.iter();

    if pinned.any(|s| s.score_id == entry.score.score_id && s.mods == entry.score.mods) {
        args.description.push_str(" 📌");
    }

    let personal = personal_idx(entry, args.personal);

    if personal.is_some() || matches!(args.global, Some((n, _)) if n == i) {
        args.description.push_str(" **(");

        if let Some(idx) = personal {
            let _ = write!(args.description, "Personal Best #{idx}");
        }

        if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == i) {
            if personal.is_some() {
                args.description.push_str(" and ");
            }

            let _ = write!(args.description, "Global Top #{idx}");
        }

        args.description.push_str(")**");
    }

    args.description.push('\n');
}

struct OptionFormat<T> {
    value: Option<T>,
}

impl<T> OptionFormat<T> {
    fn new(value: Option<T>) -> Self {
        Self { value }
    }
}

impl<T: Display> Display for OptionFormat<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.value {
            Some(ref value) => write!(f, "{value:.2}"),
            None => f.write_str("-"),
        }
    }
}

struct MissFormat<'s> {
    mode: GameMode,
    score: &'s dyn ScoreExt,
    max_combo: u32,
}

impl<'s> MissFormat<'s> {
    fn new(score: &'s ScoreSlim, max_combo: u32) -> Self {
        Self {
            mode: score.mode,
            score,
            max_combo,
        }
    }
}

impl Display for MissFormat<'_> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let miss = self.score.count_miss();

        if miss > 0 || !self.score.is_fc(self.mode, self.max_combo) {
            write!(f, "{miss}{}", Emote::Miss.text())
        } else {
            f.write_str("**FC**")
        }
    }
}
