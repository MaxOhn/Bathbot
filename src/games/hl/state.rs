use std::{mem, sync::Arc};

use eyre::Report;
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::Receiver;
use twilight_model::{
    application::interaction::MessageComponentInteraction,
    channel::embed::Embed,
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{core::Context, error::InvalidGameState, util::Authored, BotResult};

use super::{farm_map::FarmEntries, kind::GameStateKind, HlGuess, HlVersion};

pub struct GameState {
    kind: GameStateKind,
    img_url_rx: Option<Receiver<String>>,
    pub msg: Id<MessageMarker>,
    pub channel: Id<ChannelMarker>,
    pub guild: Option<Id<GuildMarker>>,
    pub current_score: u32,
    pub highscore: u32,
}

impl GameState {
    pub async fn score_pp(
        ctx: &Context,
        origin: &(dyn Authored + Sync),
        mode: GameMode,
    ) -> BotResult<Self> {
        let user = origin.user_id()?.get();
        let game_fut = GameStateKind::score_pp(ctx, mode);

        let highscore_fut = ctx
            .psql()
            .get_higherlower_highscore(user, HlVersion::ScorePp);

        let ((kind, rx), highscore) = tokio::try_join!(game_fut, highscore_fut)?;

        Ok(Self {
            kind,
            img_url_rx: Some(rx),
            msg: Id::new(1),
            channel: origin.channel_id(),
            guild: origin.guild_id(),
            current_score: 0,
            highscore,
        })
    }

    pub async fn farm_maps(ctx: &Context, origin: &(dyn Authored + Sync)) -> BotResult<Self> {
        let user = origin.user_id()?.get();

        let entries_fut = FarmEntries::new(ctx);

        let highscore_fut = ctx
            .psql()
            .get_higherlower_highscore(user, HlVersion::FarmMaps);

        let (entries, highscore) = tokio::try_join!(entries_fut, highscore_fut)?;
        let (kind, rx) = GameStateKind::farm_maps(ctx, entries).await?;

        Ok(Self {
            kind,
            img_url_rx: Some(rx),
            msg: Id::new(1),
            channel: origin.channel_id(),
            guild: origin.guild_id(),
            current_score: 0,
            highscore,
        })
    }

    pub async fn restart(self, ctx: &Context, origin: &(dyn Authored + Sync)) -> BotResult<Self> {
        let user = origin.user_id()?.get();
        let version = self.kind.version();

        let game_fut = self.kind.restart(ctx);
        let highscore_fut = ctx.psql().get_higherlower_highscore(user, version);

        let ((kind, rx), highscore) = tokio::try_join!(game_fut, highscore_fut)?;

        Ok(Self {
            kind,
            img_url_rx: Some(rx),
            msg: Id::new(1),
            channel: origin.channel_id(),
            guild: origin.guild_id(),
            current_score: 0,
            highscore,
        })
    }

    /// Set `next` to `previous` and get a new state info for `next`
    pub async fn next(&mut self, ctx: Arc<Context>) -> BotResult<()> {
        let rx = self.kind.next(ctx, self.current_score).await?;
        self.img_url_rx = Some(rx);

        Ok(())
    }

    /// Only has an image if it is the first call after [`GameState::new`] / [`GameState::next`].
    pub async fn to_embed(&mut self) -> Embed {
        let image = match self.img_url_rx.take() {
            Some(rx) => match rx.await {
                Ok(url) => url,
                Err(err) => {
                    let report = Report::new(err).wrap_err("failed to receive image url");
                    warn!("{report:?}");

                    String::new()
                }
            },
            None => {
                warn!("tried to await image again");

                String::new()
            }
        };

        self.kind.to_embed(image).footer(self.footer()).build()
    }

    pub(super) fn check_guess(&self, guess: HlGuess) -> bool {
        self.kind.check_guess(guess)
    }

    pub fn footer(&self) -> String {
        let Self {
            current_score,
            highscore,
            ..
        } = self;

        format!("Current score: {current_score} • Highscore: {highscore}")
    }

    pub fn reveal(&self, component: &mut MessageComponentInteraction) -> BotResult<Embed> {
        let mut embeds = mem::take(&mut component.message.embeds);
        let mut embed = embeds.pop().ok_or(InvalidGameState::MissingEmbed)?;
        self.kind.reveal(&mut embed);

        Ok(embed)
    }

    pub async fn new_highscore(&self, ctx: &Context, user: Id<UserMarker>) -> BotResult<bool> {
        let user = user.get();
        let version = self.kind.version();

        ctx.psql()
            .upsert_higherlower_highscore(user, version, self.current_score, self.highscore)
            .await
    }
}
