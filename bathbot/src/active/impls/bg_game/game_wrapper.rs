use std::{
    collections::{HashMap, VecDeque},
    mem,
    sync::Arc,
};

use bathbot_model::Effects;
use bathbot_psql::model::games::MapsetTagsEntries;
use bathbot_util::{constants::OSU_BASE, IntHasher, MessageBuilder};
use eyre::Result;
use tokio::sync::RwLock;
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    time::{sleep, timeout, Duration},
};
use twilight_model::{
    gateway::payload::incoming::MessageCreate,
    id::{marker::ChannelMarker, Id},
};

use super::game::{game_loop, Game, LoopResult};
use crate::{commands::fun::GameDifficulty, util::ChannelExt, Context};

const GAME_LEN: Duration = Duration::from_secs(180);

#[derive(Clone)]
pub struct BackgroundGame {
    game: Arc<RwLock<Game>>,
    tx: UnboundedSender<LoopResult>,
}

impl BackgroundGame {
    pub async fn new(
        ctx: Arc<Context>,
        channel: Id<ChannelMarker>,
        entries: MapsetTagsEntries,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut msg_stream = ctx
            .standby
            .wait_for_message_stream(channel, |event: &MessageCreate| !event.author.bot);

        let mut previous_ids = VecDeque::with_capacity(50);
        let mut scores = HashMap::with_hasher(IntHasher);

        // Initialize game
        let (game, mut img) =
            Game::new(&ctx, &entries, &mut previous_ids, effects, difficulty).await;
        let game = Arc::new(RwLock::new(game));
        let game_clone = Arc::clone(&game);

        tokio::spawn(async move {
            loop {
                let builder = MessageBuilder::new()
                    .content("Here's the next one:")
                    .attachment("bg_img.png", mem::take(&mut img));

                if let Err(err) = channel.create_message(&ctx, builder, None).await {
                    warn!(?err, "Failed to send initial bg game msg");
                }

                let result = tokio::select! {
                    // Listen for stop or restart invokes
                    option = rx.recv() => option.unwrap_or(LoopResult::Stop),
                    // Let the game run
                    result = game_loop(&mut msg_stream, &ctx, &game_clone, channel) => result,
                    // Timeout after 3 minutes
                    _ = sleep(GAME_LEN) => LoopResult::Stop,
                };

                // Process the result
                match result {
                    LoopResult::Restart => {
                        let mapset_id = game_clone.read().await.mapset_id();

                        // Send message
                        let content = format!(
                            "Mapset: {OSU_BASE}beatmapsets/{mapset_id}\n\
                            Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg"
                        );

                        if let Err(err) = channel.plain_message(&ctx, &content).await {
                            warn!(?err, "Failed to show resolve for bg game restart");
                        }
                    }
                    LoopResult::Stop => {
                        let mapset_id = game_clone.read().await.mapset_id();

                        // Send message
                        let content = format!(
                            "Mapset: {OSU_BASE}beatmapsets/{mapset_id}\n\
                            Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg\n\
                            End of game, see you next time o/"
                        );

                        if let Err(err) = channel.plain_message(&ctx, &content).await {
                            warn!(?err, "Failed to show resolve for bg game stop");
                        }

                        // Store score for winners
                        for (user, score) in scores {
                            if let Err(err) = ctx.games().bggame_increment_score(user, score).await
                            {
                                warn!("{err:?}");
                            }
                        }

                        // Then quit
                        info!(%channel, "Game finished");
                        break;
                    }
                    LoopResult::Winner(user_id) => {
                        if entries.tags.len() >= 20 {
                            *scores.entry(user_id).or_insert(0) += 1;
                        }
                    }
                }

                // Initialize next game
                let (game, img_) =
                    Game::new(&ctx, &entries, &mut previous_ids, effects, difficulty).await;
                img = img_;
                *game_clone.write().await = game;
            }

            ctx.bg_games().write(&channel).await.remove();
        });

        Self { game, tx }
    }

    pub fn stop(&self) -> Result<()> {
        self.tx
            .send(LoopResult::Stop)
            .map_err(|_| eyre!("Failed to send stop token"))
    }

    pub fn restart(&self) -> Result<()> {
        self.tx
            .send(LoopResult::Restart)
            .map_err(|_| eyre!("Failed to send restart token"))
    }

    pub async fn sub_image(&self) -> Result<Vec<u8>> {
        timeout(Duration::from_secs(1), self.game.read())
            .await?
            .sub_image()
    }

    pub async fn hint(&self) -> Result<String> {
        let game = timeout(Duration::from_secs(1), self.game.read())
            .await
            .map_err(|_| eyre!("timeout while waiting for write"))?;

        Ok(game.hint())
    }
}
