use std::{fmt::Write, time::Duration};

use bathbot_model::{
    command_fields::{ScoreEmbedImage, ScoreEmbedSettings},
    rosu_v2::user::User,
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_BASE},
    fields,
    numbers::round,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageBuilder,
};
use eyre::{Report, Result};
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component, ReactionType,
    },
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

use super::embed_builder::Settings;
use crate::{
    active::{
        impls::{CachedRender, CachedRenderData},
        pagination::{async_handle_pagination_component, handle_pagination_modal, Pages},
        ActiveMessages, BuildPage, ComponentResult, IActiveMessage,
    },
    commands::{
        osu::{OngoingRender, RenderStatus, RenderStatusInner, RENDERER_NAME},
        utility::ScoreEmbedDataWrap,
    },
    core::{buckets::BucketName, Context},
    manager::{redis::RedisData, OwnedReplayScore, ReplayScore},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        Authored, Emote, MessageExt,
    },
};

pub struct SingleScorePagination {
    pub settings: ScoreEmbedSettings,
    pub new_settings: Settings,
    scores: Box<[ScoreEmbedDataWrap]>,
    score_data: ScoreData,
    username: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,

    author: AuthorBuilder,
    content: SingleScoreContent,
}

impl SingleScorePagination {
    pub fn new(
        user: &RedisData<User>,
        scores: Box<[ScoreEmbedDataWrap]>,
        settings: ScoreEmbedSettings,
        score_data: ScoreData,
        msg_owner: Id<UserMarker>,
        content: SingleScoreContent,
    ) -> Self {
        let pages = Pages::new(1, scores.len());

        Self {
            settings,
            new_settings: Default::default(),
            scores,
            score_data,
            username: Box::from(user.username()),
            msg_owner,
            pages,
            author: user.author_builder(),
            content,
        }
    }

    pub fn set_index(&mut self, idx: usize) {
        self.pages.set_index(idx);
    }

    pub async fn async_build_page(
        &mut self,
        content: Box<str>,
        mark_idx: Option<usize>,
    ) -> Result<BuildPage> {
        let score = &*self.scores[self.pages.index()].get_mut().await?;

        let (name, value, footer_text) = self.new_settings.apply(score, self.score_data, mark_idx);

        let fields = fields![name, value, false];

        let title = format!(
            "{} - {} [{}] [{}★]",
            score.map.artist().cow_escape_markdown(),
            score.map.title().cow_escape_markdown(),
            score.map.version().cow_escape_markdown(),
            round(score.stars)
        );

        let url = format!("{OSU_BASE}b/{}", score.map.map_id());

        #[allow(unused_mut)]
        let mut description = if score.pb_idx.is_some() || score.global_idx.is_some() {
            let mut description = String::with_capacity(25);
            description.push_str("__**");

            if let Some(pb_idx) = &score.pb_idx {
                description.push_str(&pb_idx.formatted);

                if score.global_idx.is_some() {
                    description.reserve(19);
                    description.push_str(" and ");
                }
            }

            if let Some(idx) = score.global_idx {
                let _ = write!(description, "Global Top #{idx}");
            }

            description.push_str("**__");

            description
        } else {
            String::new()
        };

        #[cfg(feature = "twitch")]
        if let Some(ref data) = score.twitch {
            if !description.is_empty() {
                description.push(' ');
            }

            data.append_to_description(&score.score, &score.map, &mut description);
        }

        let mut builder = EmbedBuilder::new()
            .author(self.author.clone())
            .description(description)
            .fields(fields)
            .title(title)
            .url(url);

        match self.settings.image {
            ScoreEmbedImage::Image => builder = builder.image(score.map.cover()),
            ScoreEmbedImage::Thumbnail => builder = builder.thumbnail(score.map.thumbnail()),
            ScoreEmbedImage::None => {}
        }

        if let Some(footer_text) = footer_text {
            let emote = Emote::from(score.score.mode).url();
            let footer = FooterBuilder::new(footer_text).icon_url(emote);
            builder = builder.footer(footer);
        }

        Ok(BuildPage::new(builder, false).content(content))
    }

    async fn async_handle_component(
        &mut self,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "render" => self.handle_render_button(component).await,
            "miss_analyzer" => self.handle_miss_analyzer_button(component).await,
            _ => {
                async_handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
                    .await
                    .unwrap_or_else(ComponentResult::Err)
            }
        }
    }

    async fn handle_miss_analyzer_button(
        &mut self,
        component: &InteractionComponent,
    ) -> ComponentResult {
        let data = match self.scores[self.pages.index()].get_mut().await {
            Ok(data) => data,
            Err(err) => return ComponentResult::Err(err),
        };

        let score_id = match data.miss_analyzer.take() {
            Some(miss_analyzer) => miss_analyzer.score_id,
            None => return ComponentResult::Err(eyre!("Unexpected miss analyzer component")),
        };

        let Some(guild) = component.guild_id.map(Id::get) else {
            return ComponentResult::Err(eyre!("Missing guild id for miss analyzer button"));
        };

        let channel = component.channel_id.get();
        let msg = component.message.id.get();

        debug!(
            score_id,
            msg, channel, guild, "Sending message to miss analyzer",
        );

        let res_fut = Context::client().miss_analyzer_score_response(guild, channel, msg, score_id);

        if let Err(err) = res_fut.await {
            warn!(?err, "Failed to send miss analyzer response");
        }

        ComponentResult::BuildPage
    }

    async fn handle_render_button(&mut self, component: &InteractionComponent) -> ComponentResult {
        let data = match self.scores[self.pages.index()].get_mut().await {
            Ok(data) => data,
            Err(err) => return ComponentResult::Err(err),
        };

        let Some(replay) = data.replay.take() else {
            return ComponentResult::Err(eyre!("Unexpected render component"));
        };

        let owner = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        // Check if the score id has already been rendered
        match Context::replay().get_video_url(replay.score_id).await {
            Ok(Some(video_url)) => {
                let channel_id = component.message.channel_id;
                let username = self.username.clone();

                // Spawn in new task so that we're sure to callback the component in time
                tokio::spawn(async move {
                    let data = CachedRenderData::new_replay(replay, username);
                    let cached = CachedRender::new(data, video_url, owner);
                    let begin_fut = ActiveMessages::builder(cached).begin(channel_id);

                    if let Err(err) = begin_fut.await {
                        error!(?err, "Failed to begin cached render message");
                    }
                });

                return ComponentResult::BuildPage;
            }
            Ok(None) => {}
            Err(err) => warn!(?err),
        }

        if let Some(cooldown) = Context::check_ratelimit(owner, BucketName::Render) {
            // Put the replay back so that the button can still be used
            data.replay = Some(replay);

            return self.render_cooldown_response(component, cooldown).await;
        }

        let username = self.username.clone();

        tokio::spawn(Self::render_response(
            (component.message.id, component.message.channel_id),
            component.permissions,
            replay,
            username,
            owner,
            component.guild_id,
        ));

        ComponentResult::BuildPage
    }

    async fn render_cooldown_response(
        &mut self,
        component: &InteractionComponent,
        cooldown: i64,
    ) -> ComponentResult {
        let content = format!(
            "Rendering is on cooldown for you <@{owner}>, try again in {cooldown} seconds",
            owner = self.msg_owner
        );

        let embed = EmbedBuilder::new().description(content).color_red();
        let builder = MessageBuilder::new().embed(embed);

        let reply_fut = component.message.reply(builder, component.permissions);

        match reply_fut.await {
            Ok(_) => ComponentResult::BuildPage,
            Err(err) => {
                let wrap = "Failed to reply for render cooldown error";

                ComponentResult::Err(Report::new(err).wrap_err(wrap))
            }
        }
    }

    async fn render_response(
        orig: (Id<MessageMarker>, Id<ChannelMarker>),
        permissions: Option<Permissions>,
        replay: OwnedReplayScore,
        username: Box<str>,
        owner: Id<UserMarker>,
        guild: Option<Id<GuildMarker>>,
    ) {
        let score_id = replay.score_id;
        let mut status = RenderStatus::new_preparing_replay();
        let score = ReplayScore::from(replay);

        let msg = match orig.reply(status.as_message(), permissions).await {
            Ok(response) => match response.model().await {
                Ok(msg) => msg,
                Err(err) => return error!(?err, "Failed to get reply after render button click"),
            },
            Err(err) => return error!(?err, "Failed to reply after render button click"),
        };

        status.set(RenderStatusInner::PreparingReplay);

        if let Some(update_fut) = msg.update(status.as_message(), permissions) {
            let _ = update_fut.await;
        }

        let replay_manager = Context::replay();
        let replay_fut = replay_manager.get_replay(&score, &username);
        let settings_fut = replay_manager.get_settings(owner);

        let (replay_res, settings_res) = tokio::join!(replay_fut, settings_fut);

        let replay = match replay_res {
            Ok(Some(replay)) => replay,
            Ok(None) => {
                let content = "Looks like the replay for that score is not available";

                let embed = EmbedBuilder::new().color_red().description(content);
                let builder = MessageBuilder::new().embed(embed);

                return match msg.update(builder, permissions) {
                    Some(update_fut) => match update_fut.await {
                        Ok(_) => {}
                        Err(err) => error!(?err, "Failed to update message"),
                    },
                    None => warn!("Lacking permission to update message on error"),
                };
            }
            Err(err) => {
                let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
                let builder = MessageBuilder::new().embed(embed);

                if let Some(update_fut) = msg.update(builder, permissions) {
                    let _ = update_fut.await;
                }

                return error!(?err, "Failed to get replay");
            }
        };

        let settings = match settings_res {
            Ok(settings) => settings,
            Err(err) => {
                let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);
                let builder = MessageBuilder::new().embed(embed);

                if let Some(update_fut) = msg.update(builder, permissions) {
                    let _ = update_fut.await;
                }

                return error!(?err);
            }
        };

        status.set(RenderStatusInner::CommissioningRender);

        if let Some(update_fut) = msg.update(status.as_message(), permissions) {
            let _ = update_fut.await;
        }

        let allow_custom_skins = match guild {
            Some(guild_id) => {
                Context::guild_config()
                    .peek(guild_id, |config| config.allow_custom_skins.unwrap_or(true))
                    .await
            }
            None => true,
        };

        let skin = settings.skin(allow_custom_skins);

        let render_fut = Context::ordr()
            .expect("ordr unavailable")
            .client()
            .render_with_replay_file(&replay, RENDERER_NAME, &skin.skin)
            .options(settings.options());

        let render = match render_fut.await {
            Ok(render) => render,
            Err(err) => {
                let embed = EmbedBuilder::new().color_red().description(ORDR_ISSUE);
                let builder = MessageBuilder::new().embed(embed);

                if let Some(update_fut) = msg.update(builder, permissions) {
                    let _ = update_fut.await;
                }

                return error!(?err, "Failed to commission render");
            }
        };

        let ongoing_fut = OngoingRender::new(
            render.render_id,
            (msg, permissions),
            status,
            Some(score_id),
            owner,
        );

        ongoing_fut.await.await_render_url().await;
    }
}

impl IActiveMessage for SingleScorePagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let content = match self.content {
            SingleScoreContent::SameForAll(ref content) => content.as_str().into(),
            SingleScoreContent::OnlyForIndex { idx, ref content } if idx == self.pages.index() => {
                content.as_str().into()
            }
            SingleScoreContent::OnlyForIndex { .. } | SingleScoreContent::None => Box::default(),
        };

        Box::pin(self.async_build_page(content, None))
    }

    fn build_components(&self) -> Vec<Component> {
        let mut all_components = if self.settings.buttons.pagination {
            self.pages.components()
        } else {
            Vec::new()
        };

        let score = self.scores[self.pages.index()]
            .try_get()
            .expect("score data not yet expanded");

        if score.miss_analyzer.is_some() || score.replay.is_some() {
            let mut components = Vec::with_capacity(2);

            if score.miss_analyzer.is_some() {
                components.push(Component::Button(Button {
                    custom_id: Some("miss_analyzer".to_owned()),
                    disabled: false,
                    emoji: Some(Emote::Miss.reaction_type()),
                    label: Some("Miss analyzer".to_owned()),
                    style: ButtonStyle::Primary,
                    url: None,
                }));
            }

            if score.replay.is_some() {
                components.push(Component::Button(Button {
                    custom_id: Some("render".to_owned()),
                    disabled: false,
                    emoji: Some(ReactionType::Unicode {
                        name: "🎥".to_owned(),
                    }),
                    label: Some("Render".to_owned()),
                    style: ButtonStyle::Primary,
                    url: None,
                }));
            }

            all_components.push(Component::ActionRow(ActionRow { components }));
        }

        all_components
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(component))
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }

    fn until_timeout(&self) -> Option<Duration> {
        (!self.build_components().is_empty()).then_some(Duration::from_secs(60))
    }
}

pub enum SingleScoreContent {
    SameForAll(String),
    OnlyForIndex { idx: usize, content: String },
    None,
}
