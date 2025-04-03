use std::{
    future::ready,
    time::{Duration, Instant},
};

use bathbot_util::{EmbedBuilder, IntHasher, MessageBuilder, modal::ModalBuilder};
use enum_dispatch::enum_dispatch;
use eyre::{Report, Result, WrapErr};
use flexmap::tokio::TokioMutexMap;
use futures::future::BoxFuture;
use impls::relax::top::RelaxTopPagination;
use tokio::sync::watch::Sender;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::MessageMarker},
};

pub use self::origin::ActiveMessageOriginError;
use self::{
    builder::ActiveMessagesBuilder,
    impls::{
        BackgroundGameSetup, BadgesPagination, BookmarksPagination, CachedRender,
        ChangelogPagination, CompareMostPlayedPagination, CompareScoresPagination,
        CompareTopPagination, HelpInteractionCommand, HelpPrefixMenu, HigherLowerGame,
        LeaderboardPagination, MapPagination, MapSearchPagination, MatchComparePagination,
        MatchCostPagination, MedalCountPagination, MedalRarityPagination, MedalsCommonPagination,
        MedalsListPagination, MedalsMissingPagination, MedalsRecentPagination,
        MostPlayedPagination, NoChokePagination, OsuStatsBestPagination, OsuStatsPlayersPagination,
        OsuStatsScoresPagination, ProfileMenu, RankingCountriesPagination, RankingPagination,
        RecentListPagination, RenderSettingsActive, ScoreEmbedBuilderActive, SettingsImport,
        SimulateComponents, SingleScorePagination, SkinsPagination, SlashCommandsPagination,
        SnipeCountryListPagination, SnipeDifferencePagination, SnipePlayerListPagination,
        TopIfPagination, TopPagination, TrackListPagination,
    },
    response::ActiveResponse,
};
use crate::{
    core::{BotMetrics, Context, EventKind},
    util::{
        ComponentExt, ModalExt,
        interaction::{InteractionComponent, InteractionModal},
    },
};

pub mod impls;

mod builder;
mod origin;
mod pagination;
mod response;

#[enum_dispatch(IActiveMessage)]
pub enum ActiveMessage {
    BackgroundGameSetup,
    BadgesPagination,
    BookmarksPagination,
    CachedRender,
    ChangelogPagination,
    CompareMostPlayedPagination,
    CompareScoresPagination,
    CompareTopPagination,
    HelpInteractionCommand,
    HelpPrefixMenu,
    HigherLowerGame,
    LeaderboardPagination,
    MapPagination,
    MapSearchPagination,
    MatchComparePagination,
    MatchCostPagination,
    MedalCountPagination,
    MedalRarityPagination,
    MedalsCommonPagination,
    MedalsListPagination,
    MedalsMissingPagination,
    MedalsRecentPagination,
    MostPlayedPagination,
    NoChokePagination,
    OsuStatsBestPagination,
    OsuStatsPlayersPagination,
    OsuStatsScoresPagination,
    ProfileMenu,
    RankingPagination,
    RankingCountriesPagination,
    RecentListPagination,
    RelaxTopPagination,
    RenderSettingsActive,
    ScoreEmbedBuilderActive,
    SettingsImport,
    SimulateComponents,
    SingleScorePagination,
    SkinsPagination,
    SlashCommandsPagination,
    SnipeCountryListPagination,
    SnipeDifferencePagination,
    SnipePlayerListPagination,
    TopPagination,
    TopIfPagination,
    TrackListPagination,
}

struct FullActiveMessage {
    active_msg: ActiveMessage,
    activity_tx: Sender<()>,
}

pub struct ActiveMessages {
    inner: TokioMutexMap<Id<MessageMarker>, FullActiveMessage, IntHasher>,
}

impl Default for ActiveMessages {
    fn default() -> Self {
        Self {
            inner: TokioMutexMap::with_shard_amount_and_hasher(32, IntHasher),
        }
    }
}

impl ActiveMessages {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn builder(active_msg: impl Into<ActiveMessage>) -> ActiveMessagesBuilder {
        ActiveMessagesBuilder::new(active_msg)
    }

    pub async fn handle_component(mut component: InteractionComponent) {
        let start = Instant::now();

        EventKind::Component
            .log(&component, &component.data.custom_id)
            .await;

        let msg_id = component.message.id;
        let mut guard = Context::get().active_msgs.inner.lock(&msg_id).await;

        let Some(FullActiveMessage {
            active_msg,
            activity_tx,
        }) = guard.get_mut()
        else {
            return error!(
                name = %component.data.custom_id,
                ?component,
                "Unknown message component",
            );
        };

        async fn handle_component_inner(
            active_msg: &mut ActiveMessage,
            activity_tx: &Sender<()>,
            component: &mut InteractionComponent,
        ) {
            match active_msg.handle_component(component).await {
                ComponentResult::BuildPage => match active_msg.build_page().await {
                    Ok(build) => {
                        let mut builder = MessageBuilder::new()
                            .embed(build.embed)
                            .components(active_msg.build_components());

                        if let Some(ref content) = build.content {
                            builder = builder.content(content.as_ref());
                        }

                        if build.defer {
                            if let Err(err) = component.update(builder).await {
                                BotMetrics::inc_command_error(
                                    "component",
                                    component.data.custom_id.clone(),
                                );

                                return error!(
                                    name = %component.data.custom_id,
                                    ?err,
                                    "Failed to update component",
                                );
                            }
                        } else if let Err(err) = component.callback(builder).await {
                            BotMetrics::inc_command_error(
                                "component",
                                component.data.custom_id.clone(),
                            );

                            return error!(
                                name = %component.data.custom_id,
                                ?err,
                                "Failed to callback component",
                            );
                        }

                        let _ = activity_tx.send(());
                    }
                    Err(err) => {
                        BotMetrics::inc_command_error(
                            "component",
                            component.data.custom_id.clone(),
                        );

                        error!(
                            name = %component.data.custom_id,
                            ?err,
                            "Failed to build page for component",
                        )
                    }
                },
                ComponentResult::CreateModal(modal) => {
                    if let Err(err) = component.modal(modal).await {
                        BotMetrics::inc_command_error(
                            "component",
                            component.data.custom_id.clone(),
                        );

                        return error!(?err, "Failed to create modal");
                    }

                    let _ = activity_tx.send(());
                }
                ComponentResult::Err(err) => {
                    BotMetrics::inc_command_error("component", component.data.custom_id.clone());

                    error!(
                        name = %component.data.custom_id,
                        ?err,
                        "Failed to process component",
                    )
                }
                ComponentResult::Ignore => {}
            }
        }

        handle_component_inner(active_msg, activity_tx, &mut component).await;

        let elapsed = start.elapsed();
        BotMetrics::observe_command("component", component.data.custom_id, elapsed);
    }

    pub async fn handle_modal(mut modal: InteractionModal) {
        let start = Instant::now();

        EventKind::Modal.log(&modal, &modal.data.custom_id).await;

        let mut guard = match modal.message {
            Some(ref msg) => Context::get().active_msgs.inner.own(msg.id).await,
            None => return warn!("Received modal without message"),
        };

        let Some(FullActiveMessage {
            active_msg,
            activity_tx,
        }) = guard.get_mut()
        else {
            return error!(name = %modal.data.custom_id, ?modal, "Unknown modal");
        };

        async fn handle_modal_inner(
            active_msg: &mut ActiveMessage,
            activity_tx: &Sender<()>,
            modal: &mut InteractionModal,
        ) {
            if let Err(err) = active_msg.handle_modal(modal).await {
                BotMetrics::inc_command_error("modal", modal.data.custom_id.clone());

                return error!(name = %modal.data.custom_id, ?err, "Failed to process modal");
            }

            match active_msg.build_page().await {
                Ok(build) => {
                    let mut builder = MessageBuilder::new()
                        .embed(build.embed)
                        .components(active_msg.build_components());

                    if let Some(ref content) = build.content {
                        builder = builder.content(content.as_ref());
                    }

                    if build.defer {
                        if let Err(err) = modal.update(builder).await {
                            BotMetrics::inc_command_error("modal", modal.data.custom_id.clone());

                            return error!(
                                name = %modal.data.custom_id,
                                ?err,
                                "Failed to update modal",
                            );
                        }
                    } else if let Err(err) = modal.callback(builder).await {
                        BotMetrics::inc_command_error("modal", modal.data.custom_id.clone());

                        return error!(
                            name = %modal.data.custom_id,
                            ?err,
                            "Failed to callback modal",
                        );
                    }

                    let _ = activity_tx.send(());
                }
                Err(err) => {
                    BotMetrics::inc_command_error("modal", modal.data.custom_id.clone());

                    error!(
                        name = %modal.data.custom_id,
                        ?err,
                        "Failed to build page for modal",
                    )
                }
            }
        }

        handle_modal_inner(active_msg, activity_tx, &mut modal).await;

        let elapsed = start.elapsed();
        BotMetrics::observe_command("modal", modal.data.custom_id, elapsed);
    }

    pub async fn clear(&self) {
        self.inner.clear().await
    }

    pub async fn remove(&self, msg: Id<MessageMarker>) {
        self.remove_full(msg).await;
    }

    async fn remove_full(&self, msg: Id<MessageMarker>) -> Option<FullActiveMessage> {
        self.inner.lock(&msg).await.remove()
    }

    async fn insert(&self, msg: Id<MessageMarker>, active_msg: FullActiveMessage) {
        self.inner.own(msg).await.insert(active_msg);
    }
}

#[enum_dispatch]
pub trait IActiveMessage {
    /// The content of responses.
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>>;

    /// The components that are added to the message.
    ///
    /// Defaults to no components.
    fn build_components(&self) -> Vec<Component> {
        Vec::new()
    }

    /// What happens when the active message receives a component event.
    ///
    /// Defaults to ignoring the component.
    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        warn!(name = %component.data.custom_id, ?component, "Unknown component");

        Box::pin(ready(ComponentResult::Ignore))
    }

    /// What happens when the active message receives a modal event.
    ///
    /// Defaults to ignoring the modal.
    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        warn!(name = %modal.data.custom_id, ?modal, "Unknown modal");

        Box::pin(ready(Ok(())))
    }

    /// What happens when the message is no longer active.
    ///
    /// Defaults to removing all components.
    fn on_timeout(&mut self, response: ActiveResponse) -> BoxFuture<'_, Result<()>> {
        let builder = MessageBuilder::new().components(Vec::new());

        match response.update(builder) {
            Some(update_fut) => {
                let fut = async {
                    update_fut
                        .await
                        .map(|_| ())
                        .wrap_err("Failed to remove components")
                };

                Box::pin(fut)
            }
            None => Box::pin(ready(Err(eyre!(
                "Lacking permission to update message on timeout"
            )))),
        }
    }

    /// Duration until the message is no longer active.
    /// On `None` the message will immediatly be considered as inactive.
    ///
    /// Defaults to 1 minute.
    fn until_timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(60))
    }
}

#[derive(Clone, Default)]
pub struct BuildPage {
    embed: EmbedBuilder,
    defer: bool,
    content: Option<Box<str>>,
}

impl BuildPage {
    pub fn new(embed: EmbedBuilder, defer: bool) -> Self {
        Self {
            embed,
            defer,
            content: None,
        }
    }

    /// Wrap the [`BuildPage`] in a [`Future`](core::future::Future) that
    /// returns `Result<BuildPage>`
    pub fn boxed<'a>(self) -> BoxFuture<'a, Result<Self>> {
        Box::pin(ready(Ok(self)))
    }

    pub fn content(mut self, content: impl Into<Box<str>>) -> Self {
        self.content = Some(content.into());

        self
    }

    pub fn into_embed(self) -> EmbedBuilder {
        self.embed
    }
}

pub enum ComponentResult {
    CreateModal(ModalBuilder),
    BuildPage,
    Err(Report),
    Ignore,
}

impl ComponentResult {
    pub fn boxed<'b>(self) -> BoxFuture<'b, Self> {
        Box::pin(ready(self))
    }
}
