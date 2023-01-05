use std::{sync::Arc, time::Duration};

use bathbot_util::MessageBuilder;
use eyre::Report;
use tokio::{
    sync::oneshot::{Receiver, Sender},
    time::timeout,
};
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker, UserMarker},
    Id,
};

use crate::{core::Context, util::MessageExt};

use super::{GameState, HlComponents};

pub struct RetryState {
    pub(super) game: GameState,
    pub(super) user: Id<UserMarker>,
    pub(super) tx: Sender<()>,
}

impl RetryState {
    pub fn new(game: GameState, user: Id<UserMarker>, tx: Sender<()>) -> Self {
        Self { game, user, tx }
    }
}

const AWAIT_RETRY: Duration = Duration::from_secs(30);

pub(super) async fn await_retry(
    ctx: Arc<Context>,
    msg: Id<MessageMarker>,
    channel: Id<ChannelMarker>,
    rx: Receiver<()>,
) {
    if timeout(AWAIT_RETRY, rx).await.is_ok() {
        // Did not timeout
        return;
    }

    let components = HlComponents::disabled();
    let builder = MessageBuilder::new().components(components);

    if let Err(err) = (msg, channel).update(&ctx, &builder).await {
        let report = Report::new(err).wrap_err("failed to update retry components after timeout");
        warn!("{report:?}");
    }

    ctx.hl_retries().lock(&msg).remove();
}
