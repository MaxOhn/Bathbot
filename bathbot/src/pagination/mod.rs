use std::{sync::Arc, time::Duration};

use bathbot_util::{numbers::last_multiple, MessageBuilder};
use eyre::{Report, Result, WrapErr};
use tokio::{
    sync::watch::{self, Receiver, Sender},
    time::sleep,
};
use twilight_model::{
    application::component::{
        button::ButtonStyle, select_menu::SelectMenuOption, ActionRow, Button, Component,
        SelectMenu,
    },
    channel::embed::Embed,
    id::{
        marker::{ChannelMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    commands::osu::{TopOldCatchVersion, TopOldManiaVersion, TopOldOsuVersion, TopOldTaikoVersion},
    core::{commands::CommandOrigin, Context},
    embeds::TopOldVersion,
    util::{Emote, MessageExt},
};

pub use self::{
    badges::*, command_count::*, common::*, country_snipe_list::*, leaderboard::*, map::*,
    map_search::*, match_compare::*, medal_recent::*, medals_common::*, medals_list::*,
    medals_missing::*, most_played::*, most_played_common::*, nochoke::*, osekai_medal_count::*,
    osekai_medal_rarity::*, osustats_globals::*, osustats_list::*, osutracker_countrytop::*,
    osutracker_mappers::*, osutracker_maps::*, osutracker_mapsets::*, osutracker_mods::*,
    pages::Pages, player_snipe_list::*, profile::*, ranking::*, ranking_countries::*,
    recent_list::*, scores::*, simulate::*, sniped_difference::*, top::*, top_if::*,
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
mod simulate;
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
    Simulate(Box<SimulatePagination>),
    SnipedDiff(Box<SnipedDiffPagination>),
    Top(Box<TopPagination>),
    TopCondensed(Box<TopCondensedPagination>),
    TopIf(Box<TopIfPagination>),
    TopSingle(Box<TopSinglePagination>),
}

impl PaginationKind {
    async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        match self {
            Self::Badge(kind) => kind.build_page(ctx, pages).await,
            Self::CommandCount(kind) => Ok(kind.build_page(pages)),
            Self::Common(kind) => Ok(kind.build_page(pages)),
            Self::CountrySnipeList(kind) => Ok(kind.build_page(pages)),
            Self::Leaderboard(kind) => Ok(kind.build_page(ctx, pages).await),
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
            Self::NoChoke(kind) => Ok(kind.build_page(pages).await),
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
            Self::RecentList(kind) => Ok(kind.build_page(pages)),
            Self::Scores(kind) => Ok(kind.build_page(pages)),
            Self::Simulate(kind) => Ok(kind.build_page()),
            Self::SnipedDiff(kind) => kind.build_page(ctx, pages).await,
            Self::Top(kind) => Ok(kind.build_page(pages)),
            Self::TopCondensed(kind) => Ok(kind.build_page(pages)),
            Self::TopIf(kind) => Ok(kind.build_page(pages).await),
            Self::TopSingle(kind) => kind.build_page(ctx, pages).await,
        }
    }
}

pub struct Pagination {
    pub defer_components: bool,
    pub pages: Pages,
    pub kind: PaginationKind,
    pub component_kind: ComponentKind,
    author: Id<UserMarker>,
    tx: Sender<()>,
}

impl Pagination {
    async fn start(
        ctx: Arc<Context>,
        orig: CommandOrigin<'_>,
        builder: PaginationBuilder,
    ) -> Result<()> {
        let PaginationBuilder {
            mut kind,
            pages,
            attachment,
            content,
            start_by_callback,
            defer_components,
            component_kind,
        } = builder;

        let embed = kind
            .build_page(&ctx, &pages)
            .await
            .wrap_err("failed to build page")?;

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

        if pages.last_index() == 0 {
            return Ok(());
        }

        let response = response_raw
            .model()
            .await
            .wrap_err("failed to deserialize response")?;

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

        ctx.paginations.own(msg).await.insert(pagination);

        Ok(())
    }

    fn is_author(&self, user: Id<UserMarker>) -> bool {
        self.author == user
    }

    fn reset_timeout(&self) {
        let _ = self.tx.send(());
    }

    async fn build(&mut self, ctx: &Context) -> Result<MessageBuilder<'static>> {
        let embed = self
            .build_page(ctx)
            .await
            .wrap_err("failed to build page")?;

        let components = self.pages.components(self.component_kind);

        Ok(MessageBuilder::new().embed(embed).components(components))
    }

    async fn build_page(&mut self, ctx: &Context) -> Result<Embed> {
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
            ctx.store_msg(msg);

            loop {
                tokio::select! {
                    res = rx.changed() => if res.is_ok() {
                        continue
                    } else {
                        return
                    },
                    _ = sleep(MINUTE) => {
                        let pagination_active = ctx.paginations.lock(&msg).await.remove().is_some();
                        let msg_available = ctx.remove_msg(msg);

                        if pagination_active && msg_available {
                            let builder = MessageBuilder::new().components(Vec::new());

                            if let Some(update_fut) = (msg, channel).update(&ctx, &builder, None) {
                                if let Err(err) = update_fut.await {
                                    let err = Report::new(err).wrap_err("failed to remove components");
                                    warn!("{err:?}");
                                }
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
    pub async fn start(self, ctx: Arc<Context>, orig: CommandOrigin<'_>) -> Result<()> {
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

    pub fn simulate_components(mut self, version: TopOldVersion) -> Self {
        self.component_kind = ComponentKind::Simulate(version);

        self
    }
}

mod pages {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct Pages {
        index: usize,
        last_index: usize,
        per_page: usize,
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

        pub fn index(&self) -> usize {
            self.index
        }

        pub fn last_index(&self) -> usize {
            self.last_index
        }

        pub fn per_page(&self) -> usize {
            self.per_page
        }

        pub fn curr_page(&self) -> usize {
            self.index / self.per_page + 1
        }

        pub fn last_page(&self) -> usize {
            self.last_index / self.per_page + 1
        }

        /// Set and validate the current index to whatever `f` returns
        pub fn update(&mut self, f: impl FnOnce(&Self) -> usize) {
            self.index = self.last_index.min(f(self));
        }

        pub fn components(&self, kind: ComponentKind) -> Vec<Component> {
            match kind {
                ComponentKind::Default => self.default_components(),
                ComponentKind::MapSearch => self.map_search_components(),
                ComponentKind::Profile => self.profile_components(),
                ComponentKind::Simulate(version) => self.simulate_components(version),
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

        fn simulate_components(&self, version: TopOldVersion) -> Vec<Component> {
            macro_rules! versions {
            ( $( $label:literal, $value:literal, $version:ident = $ty:ident :: $variant:ident ;)* ) => {
                vec![
                    $(
                        SelectMenuOption {
                            default: $version == $ty::$variant,
                            description: None,
                            emoji: None,
                            label: $label.to_owned(),
                            value: $value.to_owned(),
                        },
                    )*
                ]
            }
        }

            macro_rules! button {
                ($custom_id:literal, $label:literal, $style:ident) => {
                    Button {
                        custom_id: Some($custom_id.to_owned()),
                        disabled: false,
                        emoji: None,
                        label: Some($label.to_owned()),
                        style: ButtonStyle::$style,
                        url: None,
                    }
                };
            }

            let (upper, bottom, version) = match version {
                TopOldVersion::Osu(version) => {
                    let mods = button!("sim_mods", "Mods", Primary);
                    let combo = button!("sim_combo", "Combo", Primary);
                    let acc = button!("sim_acc", "Accuracy", Primary);

                    let mut upper = vec![
                        Component::Button(mods),
                        Component::Button(combo),
                        Component::Button(acc),
                    ];

                    if let TopOldOsuVersion::September22Now = version {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }

                    let attrs = button!("sim_attrs", "Attributes", Primary);
                    upper.push(Component::Button(attrs));

                    let n300 = button!("sim_n300", "n300", Secondary);
                    let n100 = button!("sim_n100", "n100", Secondary);
                    let n50 = button!("sim_n50", "n50", Secondary);
                    let n_miss = button!("sim_miss", "Misses", Danger);

                    let bottom = vec![
                        Component::Button(n300),
                        Component::Button(n100),
                        Component::Button(n50),
                        Component::Button(n_miss),
                    ];

                    let options = versions![
                        "September 2022 - Now", "sim_osu_september22_now", version = TopOldOsuVersion::September22Now;
                        "November 2021 - September 2022", "sim_osu_november21_september22", version = TopOldOsuVersion::November21September22;
                        "July 2021 - November 2021", "sim_osu_july21_november21", version = TopOldOsuVersion::July21November21;
                        "January 2021 - July 2021", "sim_osu_january21_july21", version = TopOldOsuVersion::January21July21;
                        "February 2019 - January 2021", "sim_osu_february19_january21", version = TopOldOsuVersion::February19January21;
                        "May 2018 - February 2019", "sim_osu_may18_february19", version = TopOldOsuVersion::May18February19;
                        "April 2015 - May 2018", "sim_osu_april15_may18", version = TopOldOsuVersion::April15May18;
                        "February 2015 - April 2015", "sim_osu_february15_april15", version = TopOldOsuVersion::February15April15;
                        "July 2014 - February 2015", "sim_osu_july14_february15", version = TopOldOsuVersion::July14February15;
                        "May 2014 - July 2014", "sim_osu_may14_july14", version = TopOldOsuVersion::May14July14;
                    ];

                    let version = SelectMenu {
                        custom_id: "sim_osu_version".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options,
                        placeholder: None,
                    };

                    (upper, Some(bottom), Component::SelectMenu(version))
                }
                TopOldVersion::Taiko(version) => {
                    let mods = button!("sim_mods", "Mods", Primary);
                    let combo = button!("sim_combo", "Combo", Primary);
                    let acc = button!("sim_acc", "Accuracy", Primary);

                    let mut upper = vec![
                        Component::Button(mods),
                        Component::Button(combo),
                        Component::Button(acc),
                    ];

                    if let TopOldTaikoVersion::September22Now = version {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }

                    let attrs = button!("sim_attrs", "Attributes", Primary);
                    upper.push(Component::Button(attrs));

                    let n300 = button!("sim_n300", "n300", Secondary);
                    let n100 = button!("sim_n100", "n100", Secondary);
                    let n_miss = button!("sim_miss", "Misses", Danger);

                    let bottom = vec![
                        Component::Button(n300),
                        Component::Button(n100),
                        Component::Button(n_miss),
                    ];

                    let options = versions![
                        "September 2022 - Now", "sim_taiko_september22_now", version = TopOldTaikoVersion::September22Now;
                        "September 2020 - September 2022","sim_taiko_september20_september22", version = TopOldTaikoVersion::September20September22;
                        "March 2014 - September 2020", "sim_taiko_march14_september20", version = TopOldTaikoVersion::March14September20;
                    ];

                    let version = SelectMenu {
                        custom_id: "sim_taiko_version".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options,
                        placeholder: None,
                    };

                    (upper, Some(bottom), Component::SelectMenu(version))
                }
                TopOldVersion::Catch(version) => {
                    let mods = button!("sim_mods", "Mods", Primary);
                    let combo = button!("sim_combo", "Combo", Primary);
                    let acc = button!("sim_acc", "Accuracy", Primary);

                    let mut upper = vec![
                        Component::Button(mods),
                        Component::Button(combo),
                        Component::Button(acc),
                    ];

                    if let TopOldCatchVersion::May20Now = version {
                        let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                        upper.push(Component::Button(clock_rate));
                    }

                    let attrs = button!("sim_attrs", "Attributes", Primary);
                    upper.push(Component::Button(attrs));

                    let n_fruits = button!("sim_n300", "Fruits", Secondary);
                    let n_droplets = button!("sim_n100", "Droplets", Secondary);
                    let n_tiny_droplets = button!("sim_n50", "Tiny droplets", Secondary);
                    let n_tiny_droplet_misses =
                        button!("sim_katu", "Tiny droplet misses", Secondary);
                    let n_misses = button!("sim_miss", "Misses", Danger);

                    let bottom = vec![
                        Component::Button(n_fruits),
                        Component::Button(n_droplets),
                        Component::Button(n_tiny_droplets),
                        Component::Button(n_misses),
                        Component::Button(n_tiny_droplet_misses),
                    ];

                    let options = versions![
                        "May 2020 - Now", "sim_catch_may20_now", version = TopOldCatchVersion::May20Now;
                        "March 2014 - May 2020", "sim_catch_march14_may20", version = TopOldCatchVersion::March14May20;
                    ];

                    let version = SelectMenu {
                        custom_id: "sim_catch_version".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options,
                        placeholder: None,
                    };

                    (upper, Some(bottom), Component::SelectMenu(version))
                }
                TopOldVersion::Mania(version) => {
                    let (upper, bottom) = match version {
                        TopOldManiaVersion::March14May18 | TopOldManiaVersion::May18October22 => {
                            let mods = button!("sim_mods", "Mods", Primary);
                            let score = button!("sim_score", "Score", Primary);
                            let attrs = button!("sim_attrs", "Attributes", Primary);

                            let upper = vec![
                                Component::Button(mods),
                                Component::Button(score),
                                Component::Button(attrs),
                            ];

                            (upper, None)
                        }
                        TopOldManiaVersion::October22Now => {
                            let mods = button!("sim_mods", "Mods", Primary);
                            let acc = button!("sim_acc", "Accuracy", Primary);
                            let clock_rate = button!("sim_clock_rate", "Clock rate", Primary);
                            let attrs = button!("sim_attrs", "Attributes", Primary);
                            let n_miss = button!("sim_miss", "Misses", Danger);

                            let upper = vec![
                                Component::Button(mods),
                                Component::Button(acc),
                                Component::Button(clock_rate),
                                Component::Button(attrs),
                                Component::Button(n_miss),
                            ];

                            let n320 = button!("sim_geki", "n320", Secondary);
                            let n300 = button!("sim_n300", "n300", Secondary);
                            let n200 = button!("sim_katu", "n200", Secondary);
                            let n100 = button!("sim_n100", "n100", Secondary);
                            let n50 = button!("sim_n50", "n50", Secondary);

                            let bottom = vec![
                                Component::Button(n320),
                                Component::Button(n300),
                                Component::Button(n200),
                                Component::Button(n100),
                                Component::Button(n50),
                            ];

                            (upper, Some(bottom))
                        }
                    };

                    let options = versions![
                        "October 2022 - Now", "sim_mania_october22_now", version = TopOldManiaVersion::October22Now;
                        "May 2018 - October 2022", "sim_mania_may18_october22", version = TopOldManiaVersion::May18October22;
                        "March 2014 - May 2018", "sim_mania_march14_may18", version = TopOldManiaVersion::March14May18;
                    ];

                    let version = SelectMenu {
                        custom_id: "sim_mania_version".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options,
                        placeholder: None,
                    };

                    (upper, bottom, Component::SelectMenu(version))
                }
            };

            let upper = Component::ActionRow(ActionRow { components: upper });
            let version = Component::ActionRow(ActionRow {
                components: vec![version],
            });

            match bottom.map(|components| ActionRow { components }) {
                Some(bottom) => vec![upper, Component::ActionRow(bottom), version],
                None => vec![upper, version],
            }
        }

        fn profile_components(&self) -> Vec<Component> {
            let options = vec![
                SelectMenuOption {
                    default: self.index == 0,
                    description: Some("Compact user statistics".to_owned()),
                    emoji: None,
                    label: "Compact".to_owned(),
                    value: "compact".to_owned(),
                },
                SelectMenuOption {
                    default: self.index == 1,
                    description: Some("Extended user statistics".to_owned()),
                    emoji: None,
                    label: "User Statistics".to_owned(),
                    value: "user_stats".to_owned(),
                },
                SelectMenuOption {
                    default: self.index == 2,
                    description: Some("Min-Avg-Max values for top100 scores".to_owned()),
                    emoji: None,
                    label: "Top100 Statistics".to_owned(),
                    value: "top100_stats".to_owned(),
                },
                SelectMenuOption {
                    default: self.index == 3,
                    description: Some("Favourite mods in top100 scores".to_owned()),
                    emoji: None,
                    label: "Top100 Mods".to_owned(),
                    value: "top100_mods".to_owned(),
                },
                SelectMenuOption {
                    default: self.index == 4,
                    description: Some("Mapper appearances in top100 scores".to_owned()),
                    emoji: None,
                    label: "Top100 Mappers".to_owned(),
                    value: "top100_mappers".to_owned(),
                },
                SelectMenuOption {
                    default: self.index == 5,
                    description: Some("Mapping statistics & Kudosu".to_owned()),
                    emoji: None,
                    label: "Mapper Statistics".to_owned(),
                    value: "mapper_stats".to_owned(),
                },
            ];

            let menu = SelectMenu {
                custom_id: "profile_menu".to_owned(),
                disabled: false,
                max_values: None,
                min_values: None,
                options,
                placeholder: None,
            };

            let components = vec![Component::SelectMenu(menu)];

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
}

#[derive(Copy, Clone)]
pub enum ComponentKind {
    Default,
    MapSearch,
    Profile,
    Simulate(TopOldVersion),
}
