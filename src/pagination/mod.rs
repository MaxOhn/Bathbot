use std::{sync::Arc, time::Duration};

use tokio::{
    sync::watch::{self, Receiver, Sender},
    time::sleep,
};
use twilight_model::{
    application::component::{button::ButtonStyle, ActionRow, Button, Component},
    channel::embed::Embed,
    id::{
        marker::{ChannelMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::{commands::CommandOrigin, Context},
    util::{builder::MessageBuilder, numbers::last_multiple, Emote, MessageExt},
    BotResult,
};

pub use self::{
    badges::*, command_count::*, common::*, country_snipe_list::*, leaderboard::*, map::*,
    map_search::*, match_compare::*, medal_recent::*, medals_common::*, medals_list::*,
    medals_missing::*, most_played::*, most_played_common::*, nochoke::*, osekai_medal_count::*,
    osekai_medal_rarity::*, osustats_globals::*, osustats_list::*, osutracker_countrytop::*,
    osutracker_mappers::*, osutracker_maps::*, osutracker_mapsets::*, osutracker_mods::*,
    player_snipe_list::*, profile::*, ranking::*, ranking_countries::*, recent_list::*, scores::*,
    sniped_difference::*, top::*, top_if::*,
};

mod badges;
mod command_count;
mod common;
mod country_snipe_list;
mod leaderboard;
mod map;
mod map_search;
mod match_compare;
mod medal_recent;
mod medals_common;
mod medals_list;
mod medals_missing;
mod most_played;
mod most_played_common;
mod nochoke;
mod osekai_medal_count;
mod osekai_medal_rarity;
mod osustats_globals;
mod osustats_list;
mod osutracker_countrytop;
mod osutracker_mappers;
mod osutracker_maps;
mod osutracker_mapsets;
mod osutracker_mods;
mod player_snipe_list;
mod profile;
mod ranking;
mod ranking_countries;
mod recent_list;
mod scores;
mod sniped_difference;
mod top;
mod top_if;

pub mod components;

pub enum PaginationKind {
    Badge(Box<BadgePagination>),
    CommandCount(Box<CommandCountPagination>),
    Common(Box<CommonPagination>),
    CountrySnipeList(Box<CountrySnipeListPagination>),
    Leaderboard(Box<LeaderboardPagination>),
    Map(Box<MapPagination>),
    MapSearch(Box<MapSearchPagination>),
    MatchCompare(Box<MatchComparePagination>),
    MedalCount(Box<MedalCountPagination>),
    MedalRarity(Box<MedalRarityPagination>),
    MedalRecent(Box<MedalRecentPagination>),
    MedalsCommon(Box<MedalsCommonPagination>),
    MedalsList(Box<MedalsListPagination>),
    MedalsMissing(Box<MedalsMissingPagination>),
    MostPlayed(Box<MostPlayedPagination>),
    MostPlayedCommon(Box<MostPlayedCommonPagination>),
    NoChoke(Box<NoChokePagination>),
    OsuStatsGlobals(Box<OsuStatsGlobalsPagination>),
    OsuStatsList(Box<OsuStatsListPagination>),
    OsuTrackerCountryTop(Box<OsuTrackerCountryTopPagination>),
    OsuTrackerMappers(Box<OsuTrackerMappersPagination>),
    OsuTrackerMaps(Box<OsuTrackerMapsPagination>),
    OsuTrackerMapsets(Box<OsuTrackerMapsetsPagination>),
    OsuTrackerMods(Box<OsuTrackerModsPagination>),
    PlayerSnipeList(Box<PlayerSnipeListPagination>),
    Profile(Box<ProfilePagination>),
    Ranking(Box<RankingPagination>),
    RankingCountries(Box<RankingCountriesPagination>),
    RecentList(Box<RecentListPagination>),
    Scores(Box<ScoresPagination>),
    SnipedDiff(Box<SnipedDiffPagination>),
    Top(Box<TopPagination>),
    TopCondensed(Box<TopCondensedPagination>),
    TopIf(Box<TopIfPagination>),
}

impl PaginationKind {
    async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> BotResult<Embed> {
        match self {
            Self::Badge(kind) => kind.build_page(ctx, pages).await,
            Self::CommandCount(kind) => Ok(kind.build_page(pages)),
            Self::Common(kind) => Ok(kind.build_page(pages)),
            Self::CountrySnipeList(kind) => Ok(kind.build_page(pages)),
            Self::Leaderboard(kind) => kind.build_page(ctx, pages).await,
            Self::Map(kind) => kind.build_page(ctx, pages).await,
            Self::MapSearch(kind) => kind.build_page(ctx, pages).await,
            Self::MatchCompare(kind) => Ok(kind.build_page(pages)),
            Self::MedalCount(kind) => Ok(kind.build_page(pages)),
            Self::MedalRarity(kind) => Ok(kind.build_page(pages)),
            Self::MedalRecent(kind) => Ok(kind.build_page(pages)),
            Self::MedalsCommon(kind) => Ok(kind.build_page(pages)),
            Self::MedalsList(kind) => Ok(kind.build_page(pages)),
            Self::MedalsMissing(kind) => Ok(kind.build_page(pages)),
            Self::MostPlayed(kind) => Ok(kind.build_page(pages)),
            Self::MostPlayedCommon(kind) => Ok(kind.build_page(pages)),
            Self::NoChoke(kind) => Ok(kind.build_page(ctx, pages).await),
            Self::OsuStatsGlobals(kind) => kind.build_page(ctx, pages).await,
            Self::OsuStatsList(kind) => kind.build_page(ctx, pages).await,
            Self::OsuTrackerCountryTop(kind) => Ok(kind.build_page(pages)),
            Self::OsuTrackerMappers(kind) => Ok(kind.build_page(pages)),
            Self::OsuTrackerMaps(kind) => Ok(kind.build_page(pages)),
            Self::OsuTrackerMapsets(kind) => kind.build_page(ctx, pages).await,
            Self::OsuTrackerMods(kind) => Ok(kind.build_page(pages)),
            Self::PlayerSnipeList(kind) => kind.build_page(ctx, pages).await,
            Self::Profile(kind) => Ok(kind.build_page(ctx, pages).await),
            Self::Ranking(kind) => kind.build_page(ctx, pages).await,
            Self::RankingCountries(kind) => kind.build_page(ctx, pages).await,
            Self::RecentList(kind) => kind.build_page(ctx, pages).await,
            Self::Scores(kind) => Ok(kind.build_page(ctx, pages).await),
            Self::SnipedDiff(kind) => kind.build_page(ctx, pages).await,
            Self::Top(kind) => Ok(kind.build_page(ctx, pages).await),
            Self::TopCondensed(kind) => Ok(kind.build_page(ctx, pages).await),
            Self::TopIf(kind) => Ok(kind.build_page(pages).await),
        }
    }
}

pub struct Pagination {
    pub defer_components: bool,
    pub pages: Pages,
    author: Id<UserMarker>,
    kind: PaginationKind,
    component_kind: ComponentKind,
    tx: Sender<()>,
}

impl Pagination {
    async fn start(
        ctx: Arc<Context>,
        orig: CommandOrigin<'_>,
        builder: PaginationBuilder,
    ) -> BotResult<()> {
        let PaginationBuilder {
            mut kind,
            pages,
            attachment,
            content,
            start_by_callback,
            defer_components,
            component_kind,
        } = builder;

        let embed = kind.build_page(&ctx, &pages).await?;
        let components = pages.components(component_kind);

        let mut builder = MessageBuilder::new().embed(embed).components(components);

        if let Some((name, bytes)) = attachment {
            builder = builder.attachment(name, bytes);
        }

        if let Some(content) = content {
            builder = builder.content(content);
        }

        let response_raw = if start_by_callback {
            orig.callback_with_response(&ctx, builder).await?
        } else {
            orig.create_message(&ctx, &builder).await?
        };

        if pages.last_index == 0 {
            return Ok(());
        }

        let response = response_raw.model().await?;
        let channel = response.channel_id;
        let msg = response.id;

        let (tx, rx) = watch::channel(());
        Self::spawn_timeout(Arc::clone(&ctx), rx, msg, channel);

        let pagination = Pagination {
            author: orig.user_id()?,
            component_kind,
            defer_components,
            kind,
            pages,
            tx,
        };

        ctx.paginations.insert(msg, pagination);

        Ok(())
    }

    fn is_author(&self, user: Id<UserMarker>) -> bool {
        self.author == user
    }

    fn reset_timeout(&self) {
        let _ = self.tx.send(());
    }

    async fn build(&mut self, ctx: &Context) -> BotResult<MessageBuilder<'static>> {
        let embed = self.build_page(ctx).await?;
        let components = self.pages.components(self.component_kind);

        Ok(MessageBuilder::new().embed(embed).components(components))
    }

    async fn build_page(&mut self, ctx: &Context) -> BotResult<Embed> {
        self.kind.build_page(ctx, &self.pages).await
    }

    fn spawn_timeout(
        ctx: Arc<Context>,
        mut rx: Receiver<()>,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) {
        static MINUTE: Duration = Duration::from_secs(60);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = rx.changed() => if res.is_ok() {
                        continue
                    } else {
                        return
                    },
                    _ = sleep(MINUTE) => {
                        if ctx.paginations.remove(&msg).is_some() {
                            let builder = MessageBuilder::new().components(Vec::new());

                            if let Err(err) = (msg, channel).update(&ctx, &builder).await {
                                eprintln!("failed to remove components: {err}");
                            }
                        }

                        return;
                    },
                }
            }
        });
    }
}

pub struct PaginationBuilder {
    kind: PaginationKind,
    pages: Pages,
    attachment: Option<(String, Vec<u8>)>,
    content: Option<String>,
    start_by_callback: bool,
    defer_components: bool,
    component_kind: ComponentKind,
}

impl PaginationBuilder {
    fn new(kind: PaginationKind, pages: Pages) -> Self {
        Self {
            kind,
            pages,
            attachment: None,
            content: None,
            start_by_callback: true,
            defer_components: false,
            component_kind: ComponentKind::Default,
        }
    }

    /// Start the pagination
    pub async fn start(self, ctx: Arc<Context>, orig: CommandOrigin<'_>) -> crate::BotResult<()> {
        Pagination::start(ctx, orig, self).await
    }

    /// Add an attachment to the initial message which
    /// will stick throughout all pages.
    pub fn attachment(mut self, name: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.attachment = Some((name.into(), bytes));

        self
    }

    /// Add content to the initial message which
    /// will stick throughout all pages.
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());

        self
    }

    /// By default, the initial message will be sent by callback.
    /// This only works if the invoke originates either from a message,
    /// or from an interaction that was **not** deferred.
    ///
    /// If this method is called, the initial message will be sent
    /// through updating meaning it will work for interactions
    /// that have been deferred already.
    pub fn start_by_update(mut self) -> Self {
        self.start_by_callback = false;

        self
    }

    /// By default, the page-update message will be sent by callback.
    /// This only works if the page generation is quick enough i.e. <300ms.
    ///
    /// If this method is called, pagination components will be deferred
    /// and then after the page generation updated.
    pub fn defer_components(mut self) -> Self {
        self.defer_components = true;

        self
    }

    /// "Compact", "Medium", and "Full" button components
    pub fn profile_components(mut self) -> Self {
        self.component_kind = ComponentKind::Profile;

        self
    }

    /// "Jump start", "Step back", and "Step" button components
    pub fn map_search_components(mut self) -> Self {
        self.component_kind = ComponentKind::MapSearch;

        self
    }
}

#[derive(Clone, Debug)]
pub struct Pages {
    pub index: usize,
    last_index: usize,
    pub per_page: usize,
}

impl Pages {
    /// `per_page`: How many entries per page
    ///
    /// `amount`: How many entries in total
    pub fn new(per_page: usize, amount: usize) -> Self {
        Self {
            index: 0,
            per_page,
            last_index: last_multiple(per_page, amount),
        }
    }

    pub fn curr_page(&self) -> usize {
        self.index / self.per_page + 1
    }

    pub fn last_page(&self) -> usize {
        self.last_index / self.per_page + 1
    }

    fn components(&self, kind: ComponentKind) -> Vec<Component> {
        match kind {
            ComponentKind::Default => self.default_components(),
            ComponentKind::MapSearch => self.map_search_components(),
            ComponentKind::Profile => self.profile_components(),
        }
    }

    fn default_components(&self) -> Vec<Component> {
        if self.last_index == 0 {
            return Vec::new();
        }

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_custom = Button {
            custom_id: Some("pagination_custom".to_owned()),
            disabled: false,
            emoji: Some(Emote::MyPosition.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index == self.last_index,
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.index == self.last_index,
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let components = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(jump_custom),
            Component::Button(single_step),
            Component::Button(jump_end),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }

    fn profile_components(&self) -> Vec<Component> {
        let compact = Button {
            custom_id: Some("profile_compact".to_owned()),
            disabled: self.index == 0,
            emoji: None,
            label: Some("Compact".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let medium = Button {
            custom_id: Some("profile_medium".to_owned()),
            disabled: self.index == 1,
            emoji: None,
            label: Some("Medium".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let full = Button {
            custom_id: Some("profile_full".to_owned()),
            disabled: self.index == 2,
            emoji: None,
            label: Some("Full".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let components = vec![
            Component::Button(compact),
            Component::Button(medium),
            Component::Button(full),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }

    fn map_search_components(&self) -> Vec<Component> {
        if self.last_index == 0 {
            return Vec::new();
        }

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index == self.last_index,
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let components = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(single_step),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }
}

#[derive(Copy, Clone)]
enum ComponentKind {
    Default,
    MapSearch,
    Profile,
}
