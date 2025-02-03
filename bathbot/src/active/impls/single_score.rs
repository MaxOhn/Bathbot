use std::{borrow::Cow, fmt::Write, time::Duration};

use bathbot_model::embed_builder::{
    EmoteTextValue, HitresultsValue, MapperValue, ScoreEmbedSettings, SettingValue, SettingsImage,
    Value,
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::{GENERAL_ISSUE, ORDR_ISSUE, OSU_BASE},
    datetime::{HowLongAgoDynamic, HowLongAgoText, SecToMinSec, SHORT_NAIVE_DATETIME_FORMAT},
    fields,
    numbers::round,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageBuilder, ModsFormatter,
};
use eyre::{Report, Result};
use futures::future::BoxFuture;
use rosu_pp::model::beatmap::BeatmapAttributes;
use rosu_v2::{
    model::{GameMode, Grade},
    prelude::RankStatus,
};
use time::OffsetDateTime;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component, EmojiReactionType,
    },
    guild::Permissions,
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    active::{
        impls::{embed_builder::ValueKind, CachedRender, CachedRenderData},
        pagination::{async_handle_pagination_component, handle_pagination_modal, Pages},
        ActiveMessages, BuildPage, ComponentResult, IActiveMessage,
    },
    commands::{
        osu::{OngoingRender, RenderStatus, RenderStatusInner, RENDERER_NAME},
        utility::{ScoreEmbedData, ScoreEmbedDataWrap},
    },
    core::{buckets::BucketName, Context},
    embeds::{attachment, HitResultFormatter},
    manager::{redis::osu::CachedUser, OwnedReplayScore, ReplayScore},
    util::{
        interaction::{InteractionComponent, InteractionModal},
        osu::{GradeFormatter, ScoreFormatter},
        Authored, CachedUserExt, Emote, MessageExt,
    },
};

pub struct SingleScorePagination {
    pub settings: ScoreEmbedSettings,
    scores: Box<[ScoreEmbedDataWrap]>,
    score_data: ScoreData,
    username: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,

    author: AuthorBuilder,
    content: SingleScoreContent,
}

impl SingleScorePagination {
    pub const IMAGE_NAME: &'static str = "map_graph.png";

    pub fn new(
        user: &CachedUser,
        scores: Box<[ScoreEmbedDataWrap]>,
        settings: ScoreEmbedSettings,
        score_data: ScoreData,
        msg_owner: Id<UserMarker>,
        content: SingleScoreContent,
    ) -> Self {
        let pages = Pages::new(1, scores.len());

        Self {
            settings,
            scores,
            score_data,
            username: Box::from(user.username.as_ref()),
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
        mark_idx: MarkIndex,
    ) -> Result<BuildPage> {
        let score = &*self.scores[self.pages.index()].get_mut().await?;

        let embed = Self::apply_settings(&self.settings, score, self.score_data, mark_idx);

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

        let builder = embed
            .author(self.author.clone())
            .description(description)
            .url(url);

        Ok(BuildPage::new(builder, false).content(content))
    }

    pub fn apply_settings(
        settings: &ScoreEmbedSettings,
        data: &ScoreEmbedData,
        score_data: ScoreData,
        mark_idx: MarkIndex,
    ) -> EmbedBuilder {
        apply_settings(settings, data, score_data, mark_idx)
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

        Box::pin(self.async_build_page(content, MarkIndex::Skip))
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
                    sku_id: None,
                }));
            }

            if score.replay.is_some() {
                components.push(Component::Button(Button {
                    custom_id: Some("render".to_owned()),
                    disabled: false,
                    emoji: Some(EmojiReactionType::Unicode {
                        name: "🎥".to_owned(),
                    }),
                    label: Some("Render".to_owned()),
                    style: ButtonStyle::Primary,
                    url: None,
                    sku_id: None,
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum MarkIndex {
    /// Don't mark anything
    Skip,
    /// Mark the given index
    Some(usize),
    /// Don't mark anything but denote that this value came from the builder
    None,
}

fn apply_settings(
    settings: &ScoreEmbedSettings,
    data: &ScoreEmbedData,
    score_data: ScoreData,
    mark_idx: MarkIndex,
) -> EmbedBuilder {
    const SEP_NAME: &str = "\t";
    const SEP_VALUE: &str = " • ";

    let map_attrs = data.map.attributes().mods(data.score.mods.clone()).build();

    let mut field_name = String::new();
    let mut field_value = String::new();
    let mut footer_text = String::new();

    let mut writer = &mut field_name;

    let hide_ratio = || data.score.mode != GameMode::Mania && mark_idx == MarkIndex::Skip;

    let hide_mapper_status = || {
        matches!(
            data.map.status(),
            RankStatus::Ranked | RankStatus::Loved | RankStatus::Approved | RankStatus::Qualified
        ) && data.map.ranked_date().is_some()
            && settings
                .values
                .iter()
                .any(|value| ValueKind::from_setting(value) == ValueKind::MapRankedDate)
    };

    let first = settings.values.first().expect("at least one field");
    let next = settings.values.get(1).filter(|next| next.y == 0);

    match (&first.inner, next.map(|value| &value.inner)) {
        (
            Value::Ar | Value::Cs | Value::Hp | Value::Od,
            Some(Value::Ar | Value::Cs | Value::Hp | Value::Od),
        ) => {
            writer.push('`');

            if mark_idx == MarkIndex::Some(0) {
                writer.push('*');
            }

            let _ = match first.inner {
                Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                _ => unreachable!(),
            };

            if mark_idx == MarkIndex::Some(0) {
                writer.push('*');
            }
        }
        (Value::Ratio, _) if hide_ratio() => match next {
            Some(_) => {}
            None => writer.push_str("Ratio"), // Field name must not be empty
        },
        (Value::MapRankedDate, _) if data.map.ranked_date().is_none() => match next {
            Some(_) => {}
            None => writer.push_str("Map ranked date"), // Field name must not be empty
        },
        _ => {
            let mut value = Cow::Borrowed(first);

            if matches!(&first.inner, Value::Mapper(mapper) if mapper.with_status)
                && hide_mapper_status()
            {
                value = Cow::Owned(SettingValue {
                    inner: Value::Mapper(MapperValue { with_status: false }),
                    y: first.y,
                });
            }

            if mark_idx == MarkIndex::Some(0) {
                writer.push_str("__");
            }

            write_value(&value, data, &map_attrs, score_data, writer);

            if mark_idx == MarkIndex::Some(0) {
                writer.push_str("__");
            }
        }
    }

    for (window, i) in settings.values.windows(3).zip(1..) {
        let [prev, curr, next] = window else {
            unreachable!()
        };

        match (&prev.inner, &curr.inner, &next.inner) {
            (Value::Grade, Value::Mods, _) if prev.y == curr.y => {
                writer.push(' ');

                if mark_idx == MarkIndex::Some(i) {
                    writer.push_str("__");
                }

                let _ = write!(writer, "+{}", ModsFormatter::new(&data.score.mods));

                if mark_idx == MarkIndex::Some(i) {
                    writer.push_str("__");
                }
            }
            (
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
            ) if prev.y == curr.y && curr.y == next.y => {
                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                let _ = match curr.inner {
                    Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                    Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                    Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                    Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                    _ => unreachable!(),
                };

                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                writer.push(' ');
            }
            (
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                _,
            ) if prev.y == curr.y => {
                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                let _ = match curr.inner {
                    Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                    Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                    Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                    Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                    _ => unreachable!(),
                };

                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                if curr.y < SettingValue::FOOTER_Y {
                    writer.push('`');
                }
            }
            (
                _,
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
                Value::Ar | Value::Cs | Value::Hp | Value::Od,
            ) if curr.y == next.y => {
                if prev.y == curr.y {
                    let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                    writer.push_str(sep);
                } else if curr.y == SettingValue::FOOTER_Y {
                    writer = &mut footer_text;
                } else if prev.y == 0 {
                    writer = &mut field_value;
                } else {
                    writer.push('\n');
                }

                if curr.y < SettingValue::FOOTER_Y {
                    writer.push('`');
                }

                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                let _ = match curr.inner {
                    Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                    Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                    Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                    Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                    _ => unreachable!(),
                };

                if mark_idx == MarkIndex::Some(i) {
                    writer.push('*');
                }

                writer.push(' ');
            }
            (_, Value::Ratio, _) if hide_ratio() => {
                if prev.y == curr.y {
                } else if curr.y == SettingValue::FOOTER_Y {
                    writer = &mut footer_text;
                } else if prev.y == 0 {
                    writer = &mut field_value;
                } else {
                    writer.push('\n');
                }
            }
            (_, Value::MapRankedDate, _) if data.map.ranked_date().is_none() => {
                if prev.y == curr.y {
                    // Regular values skip the separator if ranked date came
                    // before which is wrong is Ratio is not the first value of
                    // the row so we account for that here
                    if !(ValueKind::from_setting(prev) == ValueKind::Ratio && hide_ratio()) {
                        let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                        writer.push_str(sep);
                    } else if curr.y == SettingValue::FOOTER_Y {
                        writer = &mut footer_text;
                    } else if prev.y == 0 {
                        writer = &mut field_value;
                    } else {
                        writer.push('\n');
                    }
                }
            }
            _ => {
                let mut value = Cow::Borrowed(curr);

                if matches!(&curr.inner, Value::Mapper(mapper) if mapper.with_status)
                    && hide_mapper_status()
                {
                    value = Cow::Owned(SettingValue {
                        inner: Value::Mapper(MapperValue { with_status: false }),
                        y: curr.y,
                    });
                }

                if prev.y == curr.y {
                    match &prev.inner {
                        Value::MapRankedDate if data.map.ranked_date().is_none() => {}
                        _ => {
                            let sep = if curr.y == 0 { SEP_NAME } else { SEP_VALUE };
                            writer.push_str(sep);
                        }
                    }
                } else if curr.y == SettingValue::FOOTER_Y {
                    writer = &mut footer_text;
                } else if prev.y == 0 {
                    writer = &mut field_value;
                } else {
                    writer.push('\n');
                }

                let mark = if value.y == SettingValue::FOOTER_Y {
                    "*"
                } else {
                    "__"
                };

                if mark_idx == MarkIndex::Some(i) {
                    writer.push_str(mark);
                }

                write_value(&value, data, &map_attrs, score_data, writer);

                if mark_idx == MarkIndex::Some(i) {
                    writer.push_str(mark);
                }
            }
        }
    }

    let last_idx = settings.values.len() - 1;
    let last = settings.values.get(last_idx).expect("at least one value");
    let prev = last_idx
        .checked_sub(1)
        .and_then(|idx| settings.values.get(idx));

    // A little more readable this way
    #[allow(clippy::nonminimal_bool)]
    if !(ValueKind::from_setting(last) == ValueKind::MapRankedDate
        && data.map.ranked_date().is_none())
        && !(ValueKind::from_setting(last) == ValueKind::Ratio && hide_ratio())
        && last_idx > 0
    {
        let mark = if last.y == SettingValue::FOOTER_Y {
            "*"
        } else {
            "__"
        };

        if prev.is_some_and(|prev| prev.y != last.y) {
            if last.y == SettingValue::FOOTER_Y {
                writer = &mut footer_text;
            } else if prev.is_some_and(|prev| prev.y == 0) {
                writer = &mut field_value;
            } else {
                writer.push('\n');
            }

            let mut value = Cow::Borrowed(last);

            if matches!(&last.inner, Value::Mapper(mapper) if mapper.with_status)
                && hide_mapper_status()
            {
                value = Cow::Owned(SettingValue {
                    inner: Value::Mapper(MapperValue { with_status: false }),
                    y: last.y,
                });
            }

            if mark_idx == MarkIndex::Some(last_idx) {
                writer.push_str(mark);
            }

            write_value(&value, data, &map_attrs, score_data, writer);

            if mark_idx == MarkIndex::Some(last_idx) {
                writer.push_str(mark);
            }
        } else {
            match (prev.map(|prev| &prev.inner), &last.inner) {
                (Some(Value::Grade), Value::Mods) => {
                    writer.push(' ');

                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push_str("__");
                    }

                    let _ = write!(writer, "+{}", ModsFormatter::new(&data.score.mods));

                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push_str("__");
                    }
                }
                (
                    Some(Value::Ar | Value::Cs | Value::Hp | Value::Od),
                    Value::Ar | Value::Cs | Value::Hp | Value::Od,
                ) => {
                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push('*');
                    }

                    let _ = match last.inner {
                        Value::Ar => write!(writer, "AR: {}", round(map_attrs.ar as f32)),
                        Value::Cs => write!(writer, "CS: {}", round(map_attrs.cs as f32)),
                        Value::Hp => write!(writer, "HP: {}", round(map_attrs.hp as f32)),
                        Value::Od => write!(writer, "OD: {}", round(map_attrs.od as f32)),
                        _ => unreachable!(),
                    };

                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push('*');
                    }

                    writer.push('`');
                }
                _ => {
                    match prev.map(|value| &value.inner) {
                        Some(Value::MapRankedDate) if data.map.ranked_date().is_none() => {}
                        _ => {
                            let sep = if last.y == 0 { SEP_NAME } else { SEP_VALUE };
                            writer.push_str(sep);
                        }
                    }

                    let mut value = Cow::Borrowed(last);

                    if matches!(&last.inner, Value::Mapper(mapper) if mapper.with_status)
                        && hide_mapper_status()
                    {
                        value = Cow::Owned(SettingValue {
                            inner: Value::Mapper(MapperValue { with_status: false }),
                            y: last.y,
                        });
                    }

                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push_str(mark);
                    }

                    write_value(&value, data, &map_attrs, score_data, writer);

                    if mark_idx == MarkIndex::Some(last_idx) {
                        writer.push_str(mark);
                    }
                }
            }
        }
    }

    let fields = fields![field_name, field_value, false];

    let mut title = String::with_capacity(32);

    if settings.show_artist {
        let _ = write!(title, "{} - ", data.map.artist().cow_escape_markdown());
    }

    let _ = write!(
        title,
        "{} [{}]",
        data.map.title().cow_escape_markdown(),
        data.map.version().cow_escape_markdown()
    );

    if settings.show_sr_in_title {
        let _ = write!(title, " [{}★]", round(data.stars));
    }

    let mut builder = EmbedBuilder::new().fields(fields).title(title);

    match settings.image {
        SettingsImage::Thumbnail => builder = builder.thumbnail(data.map.thumbnail()),
        SettingsImage::Image => builder = builder.image(data.map.cover()),
        SettingsImage::ImageWithStrains => {
            builder = builder.image(attachment(SingleScorePagination::IMAGE_NAME));
        }
        SettingsImage::Hide => {}
    }

    if !footer_text.is_empty() {
        let emote = Emote::from(data.score.mode).url();
        let footer = FooterBuilder::new(footer_text).icon_url(emote);
        builder = builder.footer(footer);
    }

    builder
}

const DAY: Duration = Duration::from_secs(60 * 60 * 24);

fn write_value(
    value: &SettingValue,
    data: &ScoreEmbedData,
    map_attrs: &BeatmapAttributes,
    score_data: ScoreData,
    writer: &mut String,
) {
    match &value.inner {
        Value::Grade => {
            let _ = if value.y == 0 {
                write!(
                    writer,
                    "{}",
                    GradeFormatter::new(data.score.grade, None, false),
                )
            } else if value.y == SettingValue::FOOTER_Y {
                write!(writer, "{:?}", data.score.grade)
            } else {
                write!(
                    writer,
                    "{}",
                    GradeFormatter::new(data.score.grade, Some(data.score.score_id), false),
                )
            };

            // The completion is very hard to calculate for `Catch` because
            // `n_objects` is not correct due to juicestreams so we won't
            // show it for that mode.
            let is_fail = data.score.grade == Grade::F && data.score.mode != GameMode::Catch;

            if is_fail {
                let n_objects = data.map.n_objects();

                let completion = if n_objects != 0 {
                    100 * data.score.total_hits() / n_objects
                } else {
                    100
                };

                let _ = write!(writer, "@{completion}%");
            }
        }
        Value::Mods => {
            let _ = write!(writer, "+{}", ModsFormatter::new(&data.score.mods));
        }
        Value::Score => {
            let _ = write!(writer, "{}", ScoreFormatter::new(&data.score, score_data));
        }
        Value::Accuracy => {
            let _ = write!(writer, "{}%", round(data.score.accuracy));
        }
        Value::ScoreDate => {
            let score_date = data.score.ended_at;

            if value.y == SettingValue::FOOTER_Y {
                writer.push_str("Played ");

                if OffsetDateTime::now_utc() < score_date + DAY {
                    let _ = write!(writer, "{}", HowLongAgoText::new(&score_date));
                } else {
                    writer.push_str(&score_date.format(&SHORT_NAIVE_DATETIME_FORMAT).unwrap());
                    writer.push_str(" UTC");
                }
            } else {
                let _ = write!(writer, "{}", HowLongAgoDynamic::new(&score_date));
            }
        }
        Value::Pp(pp) => {
            let bold = if value.y < SettingValue::FOOTER_Y {
                "**"
            } else {
                ""
            };
            let tilde = if value.y < SettingValue::FOOTER_Y {
                "~~"
            } else {
                ""
            };

            let _ = write!(writer, "{bold}{:.2}", data.score.pp);

            let _ = match (pp.max, data.if_fc_pp.filter(|_| pp.if_fc), pp.max_if_fc) {
                (true, Some(if_fc_pp), _) => {
                    write!(
                        writer,
                        "{bold}/{max:.2}PP {tilde}({if_fc_pp:.2}pp){tilde}",
                        max = data.max_pp.max(data.score.pp)
                    )
                }
                (true, None, _) | (false, None, true) => {
                    write!(writer, "{bold}/{:.2}PP", data.max_pp.max(data.score.pp))
                }
                (false, Some(if_fc_pp), _) => {
                    write!(writer, "pp{bold} {tilde}({if_fc_pp:.2}pp){tilde}")
                }
                (false, None, false) => write!(writer, "pp{bold}"),
            };
        }
        Value::Combo(combo) => {
            if value.y < SettingValue::FOOTER_Y {
                writer.push_str("**");
            }

            let _ = write!(writer, "{}x", data.score.max_combo);

            if value.y < SettingValue::FOOTER_Y {
                writer.push_str("**");
            }

            if combo.max {
                let _ = write!(writer, "/{}x", data.max_combo);
            }
        }
        Value::Hitresults(hitresults) => {
            let _ = match hitresults {
                HitresultsValue::Full => write!(
                    writer,
                    "{}",
                    HitResultFormatter::new(data.score.mode, &data.score.statistics)
                ),
                HitresultsValue::OnlyMisses if value.y < SettingValue::FOOTER_Y => {
                    write!(writer, "{}{}", data.score.statistics.miss, Emote::Miss)
                }
                HitresultsValue::OnlyMisses => {
                    write!(writer, "{} miss", data.score.statistics.miss)
                }
            };
        }
        Value::Ratio => {
            let mut ratio = data.score.statistics.perfect as f32;

            let against: u8 = if data.score.statistics.great > 0 {
                ratio /= data.score.statistics.great as f32;

                1
            } else {
                0
            };

            let _ = write!(writer, "{ratio:.2}:{against}");
        }
        Value::Stars => {
            let _ = write!(writer, "{}★", round(data.stars));
        }
        Value::Length => {
            let clock_rate = map_attrs.clock_rate as f32;
            let seconds_drain = (data.map.seconds_drain() as f32 / clock_rate) as u32;

            if value.y < SettingValue::FOOTER_Y {
                writer.push('`');
            }

            let _ = write!(writer, "{}", SecToMinSec::new(seconds_drain).pad_secs());

            if value.y < SettingValue::FOOTER_Y {
                writer.push('`');
            }
        }
        Value::Ar | Value::Cs | Value::Hp | Value::Od => {
            if value.y < SettingValue::FOOTER_Y {
                writer.push('`');
            }

            let mut write = |name, value| write!(writer, "{name}: {}", round(value as f32));

            let _ = match &value.inner {
                Value::Ar => write("AR", map_attrs.ar),
                Value::Cs => write("CS", map_attrs.cs),
                Value::Hp => write("HP", map_attrs.hp),
                Value::Od => write("OD", map_attrs.od),
                _ => unreachable!(),
            };

            if value.y < SettingValue::FOOTER_Y {
                writer.push('`');
            }
        }
        Value::Bpm(emote_text) => {
            let clock_rate = map_attrs.clock_rate as f32;
            let bpm = round(data.map.bpm() * clock_rate);

            if value.y < SettingValue::FOOTER_Y {
                writer.push_str("**");
            }

            let _ = match emote_text {
                EmoteTextValue::Emote if value.y < SettingValue::FOOTER_Y => {
                    write!(writer, "{} {bpm}", Emote::Bpm)
                }
                EmoteTextValue::Text | EmoteTextValue::Emote => write!(writer, "{bpm} BPM"),
            };

            if value.y < SettingValue::FOOTER_Y {
                writer.push_str("**");
            }
        }
        Value::CountObjects(emote_text) => {
            let n = data.map.n_objects();

            let _ = match emote_text {
                EmoteTextValue::Emote if value.y < SettingValue::FOOTER_Y => {
                    write!(writer, "{} {n}", Emote::CountObjects)
                }
                EmoteTextValue::Text | EmoteTextValue::Emote => {
                    write!(
                        writer,
                        "{n} object{plural}",
                        plural = if n == 1 { "" } else { "s" }
                    )
                }
            };
        }
        Value::CountSliders(emote_text) => {
            let n = data.map.n_sliders();

            let _ = match emote_text {
                EmoteTextValue::Emote if value.y < SettingValue::FOOTER_Y => {
                    write!(writer, "{} {n}", Emote::CountSliders)
                }
                EmoteTextValue::Text | EmoteTextValue::Emote => {
                    write!(
                        writer,
                        "{n} slider{plural}",
                        plural = if n == 1 { "" } else { "s" }
                    )
                }
            };
        }
        Value::CountSpinners(emote_text) => {
            let n = data.map.n_spinners();

            let _ = match emote_text {
                EmoteTextValue::Emote if value.y < SettingValue::FOOTER_Y => {
                    write!(writer, "{} {n}", Emote::CountSpinners)
                }
                EmoteTextValue::Text | EmoteTextValue::Emote => {
                    write!(
                        writer,
                        "{n} spinner{plural}",
                        plural = if n == 1 { "" } else { "s" }
                    )
                }
            };
        }
        Value::MapRankedDate => {
            if let Some(ranked_date) = data.map.ranked_date() {
                let _ = write!(writer, "{:?} ", data.map.status());

                if OffsetDateTime::now_utc() < ranked_date + DAY {
                    let _ = if value.y == SettingValue::FOOTER_Y {
                        write!(writer, "{}", HowLongAgoText::new(&ranked_date))
                    } else {
                        write!(writer, "{}", HowLongAgoDynamic::new(&ranked_date))
                    };
                } else if value.y == SettingValue::FOOTER_Y {
                    writer.push_str(&ranked_date.format(&SHORT_NAIVE_DATETIME_FORMAT).unwrap());
                    writer.push_str(" UTC");
                } else {
                    let _ = write!(writer, "<t:{}:f>", ranked_date.unix_timestamp());
                }
            }
        }
        Value::Mapper(mapper) => {
            let creator = data.map.creator();

            let _ = if mapper.with_status {
                write!(writer, "{:?} mapset by {creator}", data.map.status())
            } else {
                write!(writer, "Mapset by {creator}")
            };
        }
    }
}
