use std::{future::ready, sync::Arc, time::Duration};

use bathbot_util::{modal::ModalBuilder, EmbedBuilder, IntHasher, MessageBuilder};
use enum_dispatch::enum_dispatch;
use eyre::{Report, Result, WrapErr};
use flexmap::tokio::TokioMutexMap;
use futures::future::BoxFuture;
use tokio::sync::watch::Sender;
use twilight_model::{
    channel::message::Component,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

pub use self::origin::ActiveMessageOriginError;
use self::{
    builder::ActiveMessagesBuilder,
    impls::{
        BackgroundGameSetup, BadgesPagination, BookmarksPagination, CachedRender,
        ChangelogPagination, CompareMostPlayedPagination, CompareScoresPagination,
        CompareTopPagination, CountryTopPagination, EditOnTimeout, HelpInteractionCommand,
        HelpPrefixMenu, HigherLowerGame, LeaderboardPagination, MapPagination, MapSearchPagination,
        MatchComparePagination, MedalCountPagination, MedalRarityPagination,
        MedalsCommonPagination, MedalsListPagination, MedalsMissingPagination,
        MedalsRecentPagination, MostPlayedPagination, NoChokePagination, OsuStatsBestPagination,
        OsuStatsPlayersPagination, OsuStatsScoresPagination, PopularMappersPagination,
        PopularMapsPagination, PopularMapsetsPagination, PopularModsPagination, ProfileMenu,
        RankingCountriesPagination, RankingPagination, RecentListPagination, RenderSettingsActive,
        ScoresMapPagination, ScoresServerPagination, ScoresUserPagination, SettingsImport,
        SimulateComponents, SkinsPagination, SlashCommandsPagination, SnipeCountryListPagination,
        SnipeDifferencePagination, SnipePlayerListPagination, TopIfPagination, TopPagination,
    },
};
use crate::{
    core::{Context, EventKind},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        ComponentExt, MessageExt, ModalExt,
    },
};

pub mod impls;

mod builder;
mod origin;
mod pagination;

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
    CountryTopPagination,
    EditOnTimeout,
    HelpInteractionCommand,
    HelpPrefixMenu,
    HigherLowerGame,
    LeaderboardPagination,
    MapPagination,
    MapSearchPagination,
    MatchComparePagination,
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
    PopularMappersPagination,
    PopularMapsPagination,
    PopularMapsetsPagination,
    PopularModsPagination,
    ProfileMenu,
    RankingPagination,
    RankingCountriesPagination,
    RecentListPagination,
    RenderSettingsActive,
    ScoresMapPagination,
    ScoresServerPagination,
    ScoresUserPagination,
    SettingsImport,
    SimulateComponents,
    SkinsPagination,
    SlashCommandsPagination,
    SnipeCountryListPagination,
    SnipeDifferencePagination,
    SnipePlayerListPagination,
    TopPagination,
    TopIfPagination,
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

    pub async fn handle_component(ctx: Arc<Context>, mut component: InteractionComponent) {
        EventKind::Component
            .log(&ctx, &component, &component.data.custom_id)
            .await;

        let msg_id = component.message.id;
        let mut guard = ctx.active_msgs.inner.lock(&msg_id).await;

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

        match active_msg
            .handle_component(Arc::clone(&ctx), &mut component)
            .await
        {
            ComponentResult::BuildPage => match active_msg.build_page(Arc::clone(&ctx)).await {
                Ok(build) => {
                    let mut builder = MessageBuilder::new()
                        .embed(build.embed)
                        .components(active_msg.build_components());

                    if let Some(ref content) = build.content {
                        builder = builder.content(content.as_ref());
                    }

                    if build.defer {
                        if let Some(fut) = component.update(&ctx, builder) {
                            if let Err(err) = fut.await {
                                return error!(
                                    name = %component.data.custom_id,
                                    ?err,
                                    "Failed to update component",
                                );
                            }
                        } else {
                            return warn!("Lacking permission to update message through component");
                        }
                    } else if let Err(err) = component.callback(&ctx, builder).await {
                        return error!(
                            name = %component.data.custom_id,
                            ?err,
                            "Failed to callback component",
                        );
                    }

                    let _ = activity_tx.send(());
                }
                Err(err) => error!(
                    name = %component.data.custom_id,
                    ?err,
                    "Failed to build page for component",
                ),
            },
            ComponentResult::CreateModal(modal) => {
                if let Err(err) = component.modal(&ctx, modal).await {
                    return error!(?err, "Failed to create modal");
                }

                let _ = activity_tx.send(());
            }
            ComponentResult::Err(err) => {
                error!(
                    name = %component.data.custom_id,
                    ?err,
                    "Failed to process component",
                )
            }
            ComponentResult::Ignore => {}
        }
    }

    pub async fn handle_modal(ctx: Arc<Context>, mut modal: InteractionModal) {
        EventKind::Modal
            .log(&ctx, &modal, &modal.data.custom_id)
            .await;

        let mut guard = match modal.message {
            Some(ref msg) => ctx.active_msgs.inner.own(msg.id).await,
            None => return warn!("Received modal without message"),
        };

        let Some(FullActiveMessage {
            active_msg,
            activity_tx,
        }) = guard.get_mut()
        else {
            return error!(name = %modal.data.custom_id, ?modal, "Unknown modal");
        };

        if let Err(err) = active_msg.handle_modal(&ctx, &mut modal).await {
            return error!(name = %modal.data.custom_id, ?err, "Failed to process modal");
        }

        match active_msg.build_page(Arc::clone(&ctx)).await {
            Ok(build) => {
                let mut builder = MessageBuilder::new()
                    .embed(build.embed)
                    .components(active_msg.build_components());

                if let Some(ref content) = build.content {
                    builder = builder.content(content.as_ref());
                }

                if build.defer {
                    if let Some(fut) = modal.update(&ctx, builder) {
                        if let Err(err) = fut.await {
                            return error!(
                                name = %modal.data.custom_id,
                                ?err,
                                "Failed to update modal",
                            );
                        }
                    } else {
                        return warn!("Lacking permission to update message through modal");
                    }
                } else if let Err(err) = modal.callback(&ctx, builder).await {
                    return error!(
                        name = %modal.data.custom_id,
                        ?err,
                        "Failed to callback modal",
                    );
                }

                let _ = activity_tx.send(());
            }
            Err(err) => error!(
                name = %modal.data.custom_id,
                ?err,
                "Failed to build page for modal",
            ),
        }
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
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>>;

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
        _ctx: Arc<Context>,
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
        _ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        warn!(name = %modal.data.custom_id, ?modal, "Unknown modal");

        Box::pin(ready(Ok(())))
    }

    /// What happens when the message is no longer active.
    ///
    /// Defaults to removing all components.
    fn on_timeout<'a>(
        &'a mut self,
        ctx: &'a Context,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
    ) -> BoxFuture<'a, Result<()>> {
        let builder = MessageBuilder::new().components(Vec::new());

        match (msg, channel).update(ctx, builder, None) {
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
