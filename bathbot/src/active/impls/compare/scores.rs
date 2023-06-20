use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{rosu_v2::user::User, ScoreSlim};
use bathbot_util::{
    constants::{AVATAR_URL, OSU_BASE},
    datetime::HowLongAgoDynamic,
    numbers::{round, WithComma},
    CowUtils, EmbedBuilder, FooterBuilder, MessageOrigin, ScoreExt,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::{GameMode, Score};
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{CompareEntry, GlobalIndex},
    core::{BotConfig, Context},
    embeds::{ComboFormatter, HitResultFormatter},
    manager::{redis::RedisData, OsuMap},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::PersonalBestIndex,
        Emote,
    },
};

#[derive(PaginationBuilder)]
pub struct CompareScoresPagination {
    user: RedisData<User>,
    map: OsuMap,
    #[pagination(per_page = 10)]
    entries: Box<[CompareEntry]>,
    pinned: Box<[Score]>,
    personal: Box<[Score]>,
    global_idx: Option<GlobalIndex>,
    pp_idx: usize,
    origin: MessageOrigin,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for CompareScoresPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let global_idx = self
            .global_idx
            .as_ref()
            .filter(|global| {
                (pages.index()..pages.index() + pages.per_page()).contains(&global.idx_in_entries)
            })
            .map(|global| {
                let factor = global.idx_in_entries / pages.per_page();
                let new_idx = global.idx_in_entries - factor * pages.per_page();

                (new_idx, global.idx_in_map_lb)
            });

        let page = pages.curr_page();
        let pages = pages.last_page();

        let mut description = String::with_capacity(512);
        let pp_idx = (page == self.pp_idx / 10 + 1).then_some(self.pp_idx % 10);
        let mut args = WriteArgs::new(
            &mut description,
            &self.pinned,
            &self.personal,
            global_idx,
            pp_idx,
        );

        let mut entries = entries.iter();

        if page == 1 {
            if let Some(entry) = entries.next() {
                let personal_best = PersonalBestIndex::new(
                    &entry.score,
                    self.map.map_id(),
                    self.map.status(),
                    args.personal,
                )
                .into_embed_description(&self.origin);

                if personal_best.is_some() || matches!(args.global, Some((0, _))) {
                    args.description.push_str("__**");

                    if let Some(ref desc) = personal_best {
                        args.description.push_str(desc);
                    }

                    if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == 0) {
                        if personal_best.is_some() {
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

                let _ = write!(
                    args.description,
                    "{grade} **+{mods}** [{stars:.2}★] • {score} • {acc}%\n\
                    {pp_format}**{pp:.2}**{pp_format}/{max_pp:.2}PP • {combo}",
                    grade = BotConfig::get().grade(entry.score.grade),
                    mods = entry.score.mods,
                    stars = entry.stars,
                    score = WithComma::new(entry.score.score),
                    acc = round(entry.score.accuracy),
                    pp_format = if pp_idx == Some(0) { "" } else { "~~" },
                    pp = entry.score.pp,
                    max_pp = entry.score.pp.max(entry.max_pp),
                    combo = ComboFormatter::new(entry.score.max_combo, Some(entry.max_combo)),
                );

                if let Some(ref if_fc) = entry.if_fc {
                    let _ = writeln!(args.description, " • __If FC:__ *{:.2}pp*", if_fc.pp);
                } else {
                    args.description.push('\n');
                }

                let _ = write!(
                    args.description,
                    "{hits} {timestamp}",
                    hits =
                        HitResultFormatter::new(entry.score.mode, entry.score.statistics.clone()),
                    timestamp = HowLongAgoDynamic::new(&entry.score.ended_at)
                );

                if let Some(score_id) = entry.score.score_id.filter(|_| entry.has_replay) {
                    let _ = write!(
                        args.description,
                        " • [Replay]({OSU_BASE}scores/{mode}/{score_id}/download)",
                        mode = entry.score.mode
                    );
                }

                args.description.push('\n');

                if let Some(entry) = entries.next() {
                    args.description
                        .push_str("\n__Other scores on the beatmap:__\n");
                    write_compact_entry(&mut args, 1, entry, &self.map, &self.origin);
                }
            }
        }

        for (entry, i) in entries.zip(2..) {
            write_compact_entry(&mut args, i, entry, &self.map, &self.origin);
        }

        if args.description.is_empty() {
            args.description.push_str("No scores found");
        }

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} mapset of {creator}",
            status = self.map.status(),
            creator = self.map.creator(),
        );

        let footer_icon = format!(
            "{AVATAR_URL}{creator_id}",
            creator_id = self.map.creator_id()
        );
        let footer = FooterBuilder::new(footer_text).icon_url(footer_icon);

        let mut title_text = String::with_capacity(32);

        let _ = write!(
            title_text,
            "{artist} - {title} [{version}]",
            artist = self.map.artist().cow_escape_markdown(),
            title = self.map.title().cow_escape_markdown(),
            version = self.map.version().cow_escape_markdown()
        );

        if self.map.mode() == GameMode::Mania {
            let _ = write!(title_text, "[{}K] ", self.map.cs() as u32);
        }

        let url = format!("{OSU_BASE}b/{}", self.map.map_id());

        let embed = EmbedBuilder::new()
            .author(self.user.author_builder())
            .description(description)
            .footer(footer)
            .thumbnail(self.map.thumbnail())
            .title(title_text)
            .url(url);

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(ctx, component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, false, &mut self.pages)
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

fn write_compact_entry(
    args: &mut WriteArgs<'_>,
    i: usize,
    entry: &CompareEntry,
    map: &OsuMap,
    origin: &MessageOrigin,
) {
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
        miss = MissFormat::new(&entry.score, entry.max_combo),
        timestamp = HowLongAgoDynamic::new(&entry.score.ended_at),
    );

    let mut pinned = args.pinned.iter();

    if pinned.any(|s| s.score_id == entry.score.score_id && s.mods == entry.score.mods) {
        args.description.push_str(" 📌");
    }

    let personal_best =
        PersonalBestIndex::new(&entry.score, map.map_id(), map.status(), args.personal)
            .into_embed_description(origin);

    if personal_best.is_some() || matches!(args.global, Some((n, _)) if n == i) {
        args.description.push_str(" **(");

        if let Some(ref desc) = personal_best {
            args.description.push_str(desc);
        }

        if let Some((_, idx)) = args.global.filter(|(idx, _)| *idx == i) {
            if personal_best.is_some() {
                args.description.push_str(" and ");
            }

            let _ = write!(args.description, "Global Top #{idx}");
        }

        args.description.push_str(")**");
    }

    args.description.push('\n');
}

struct MissFormat<'s> {
    mode: GameMode,
    score: &'s ScoreSlim,
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
        let miss = self.score.statistics.count_miss;

        if miss > 0 || !self.score.is_fc(self.mode, self.max_combo) {
            write!(f, "{miss}{}", Emote::Miss.text())
        } else {
            f.write_str("**FC**")
        }
    }
}
