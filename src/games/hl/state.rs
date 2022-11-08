use std::{mem, sync::Arc};

use eyre::{ContextCompat, Report, Result, WrapErr};
use rosu_v2::prelude::GameMode;
use tokio::sync::oneshot::Receiver;
use twilight_model::{
    channel::embed::Embed,
    id::{
        marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    core::Context,
    util::{interaction::InteractionComponent, Authored},
};

use super::{kind::GameStateKind, HlGuess, HlVersion};

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
    ) -> Result<Self> {
        let user = origin.user_id()?;

        let game_fut = GameStateKind::score_pp(ctx, mode);
        let highscore_fut = ctx.games().higherlower_highscore(user, HlVersion::ScorePp);

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

    pub async fn farm_maps(ctx: &Context, origin: &(dyn Authored + Sync)) -> Result<Self> {
        let user = origin.user_id()?;

        let entries_fut = ctx.redis().osutracker_counts();
        let highscore_fut = ctx.games().higherlower_highscore(user, HlVersion::FarmMaps);

        let (entries_res, highscore_res) = tokio::join!(entries_fut, highscore_fut);
        let highscore = highscore_res.wrap_err("failed to get highscore from database")?;

        let (kind, rx) = GameStateKind::farm_maps(ctx, entries_res?)
            .await
            .wrap_err("failed to create farm maps game state")?;

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

    pub async fn restart(self, ctx: &Context, origin: &(dyn Authored + Sync)) -> Result<Self> {
        let user = origin.user_id()?;
        let version = self.kind.version();

        let game_fut = self.kind.restart(ctx);
        let highscore_fut = ctx.games().higherlower_highscore(user, version);

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
    pub async fn next(&mut self, ctx: Arc<Context>) -> Result<()> {
        let rx = self
            .kind
            .next(ctx, self.current_score)
            .await
            .wrap_err("failed to get next game")?;

        self.img_url_rx = Some(rx);

        Ok(())
    }

    /// Only has an image if it is the first call after initialization / [`GameState::next`].
    pub async fn make_embed(&mut self) -> Embed {
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

    pub fn reveal(&self, component: &mut InteractionComponent) -> Result<Embed> {
        let mut embeds = mem::take(&mut component.message.embeds);
        let mut embed = embeds.pop().wrap_err("missing embed")?;
        self.kind.reveal(&mut embed);

        Ok(embed)
    }

    pub async fn new_highscore(&self, ctx: &Context, user: Id<UserMarker>) -> Result<bool> {
        ctx.games()
            .upsert_higherlower_score(user, self.kind.version(), self.current_score)
            .await
            .wrap_err("failed to upsert higherlower score")
    }
}
