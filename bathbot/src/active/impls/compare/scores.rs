use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{ScoreSlim, embed_builder::ScoreEmbedSettings};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    CowUtils, EmbedBuilder, FooterBuilder, ModsFormatter, ScoreExt, constants::OSU_BASE,
    datetime::HowLongAgoDynamic, numbers::round,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::prelude::{GameMode, Score};
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        impls::{MarkIndex, SingleScorePagination},
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::utility::ScoreEmbedData,
    manager::{OsuMap, redis::osu::CachedUser},
    util::{
        CachedUserExt, Emote,
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeFormatter,
    },
};

#[derive(PaginationBuilder)]
pub struct CompareScoresPagination {
    user: CachedUser,
    map: OsuMap,
    settings: ScoreEmbedSettings,
    #[pagination(per_page = 10)]
    entries: Box<[ScoreEmbedData]>,
    pinned: Box<[Score]>,
    pp_idx: usize,
    score_data: ScoreData,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for CompareScoresPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let end_idx = self.entries.len().min(pages.index() + pages.per_page());
        let entries = &self.entries[pages.index()..end_idx];

        let page = pages.curr_page();
        let pages = pages.last_page();

        let mut description = String::with_capacity(512);
        let pp_idx = (page == self.pp_idx / 10 + 1).then_some(self.pp_idx % 10);

        let footer_text = format!(
            "Page {page}/{pages} • {status:?} mapset by {creator}",
            status = self.map.status(),
            creator = self.map.creator(),
        );
        let footer_icon = Emote::from(self.map.mode()).url();
        let footer = FooterBuilder::new(footer_text).icon_url(footer_icon);

        let mut title = String::with_capacity(32);

        if self.settings.show_artist {
            let _ = write!(title, "{} - ", self.map.artist().cow_escape_markdown());
        }

        let _ = write!(
            title,
            "{} [{}]",
            self.map.title().cow_escape_markdown(),
            self.map.version().cow_escape_markdown(),
        );

        let mut embed = EmbedBuilder::new()
            .author(self.user.author_builder(false))
            .footer(footer)
            .title(title)
            .url(format!("{OSU_BASE}b/{}", self.map.map_id()));

        embed = if let Some(entry) = entries.first() {
            let mut applied_settings = SingleScorePagination::apply_settings(
                &self.settings,
                entry,
                self.score_data,
                MarkIndex::Skip,
            );

            if page == 1 {
                if entry.pb_idx.is_some() || entry.global_idx.is_some() {
                    description.push_str("__**");

                    if let Some(ref pb) = entry.pb_idx {
                        description.push_str(&pb.formatted);
                    }

                    if let Some(idx) = entry.global_idx {
                        if entry.pb_idx.is_some() {
                            description.push_str(" and ");
                        }

                        let _ = write!(description, "Global Top #{idx}");
                    }

                    description.push_str("**__");
                }

                if self.pinned.iter().any(|s| s.id == entry.score.score_id) {
                    description.push_str(" 📌");
                }

                if !description.is_empty() {
                    description.push('\n');
                }

                if entries.len() > 1 {
                    let field = applied_settings.fields.pop().expect("at least one field");
                    description.push_str(&field.name.replace('\t', " • "));
                    description.push('\n');
                    description.push_str(&field.value);
                    description.push_str("\n\n__Other scores on the beatmap:__\n");

                    for (entry, i) in entries[1..].iter().zip(1..) {
                        write_compact_entry(&mut description, pp_idx, &self.pinned, i, entry);
                    }
                } else {
                    embed = embed.fields(applied_settings.fields);

                    if let Some(footer) = applied_settings.footer {
                        embed = embed.footer(footer);
                    }
                }

                if let Some(title) = applied_settings.title {
                    embed = embed.title(title);
                }
            } else {
                for (i, entry) in entries.iter().enumerate() {
                    write_compact_entry(&mut description, pp_idx, &self.pinned, i, entry);
                }
            }

            if let Some(thumbnail) = applied_settings.thumbnail_url {
                embed = embed.thumbnail(thumbnail);
            } else if let Some(image) = applied_settings.image_url {
                embed = embed.image(image);
            }

            embed.description(description)
        } else {
            embed
                .description("No scores found")
                .thumbnail(self.map.thumbnail())
        };

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}

fn write_compact_entry(
    writer: &mut String,
    pp_idx: Option<usize>,
    pinned: &[Score],
    i: usize,
    entry: &ScoreEmbedData,
) {
    let _ = write!(
        writer,
        "{grade} **+{mods}** [{stars:.2}★] {pp_format}{pp:.2}pp{pp_format} \
        ({acc}%) {combo}x • {miss} {timestamp}",
        grade = GradeFormatter::new(
            entry.score.grade,
            Some(entry.score.score_id),
            entry.score.is_legacy()
        ),
        mods = ModsFormatter::new(&entry.score.mods),
        stars = entry.stars,
        pp_format = if pp_idx == Some(i) { "**" } else { "~~" },
        pp = entry.score.pp,
        acc = round(entry.score.accuracy),
        combo = entry.score.max_combo,
        miss = MissFormat::new(&entry.score, entry.max_combo),
        timestamp = HowLongAgoDynamic::new(&entry.score.ended_at),
    );

    if pinned.iter().any(|s| s.id == entry.score.score_id) {
        writer.push_str(" 📌");
    }

    if entry.pb_idx.is_some() || entry.global_idx.is_some() {
        writer.push_str(" **(");

        if let Some(ref pb) = entry.pb_idx {
            writer.push_str(&pb.formatted);
        }

        if let Some(idx) = entry.global_idx {
            if entry.pb_idx.is_some() {
                writer.push_str(" and ");
            }

            let _ = write!(writer, "Global Top #{idx}");
        }

        writer.push_str(")**");
    }

    writer.push('\n');
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
        let miss = self.score.statistics.miss;

        if miss > 0 || !self.score.is_fc(self.mode, self.max_combo) {
            write!(f, "{miss}{}", Emote::Miss)
        } else {
            f.write_str("**FC**")
        }
    }
}
