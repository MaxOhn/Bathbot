use super::{game_loop, Game, GameResult, LoopResult};
use crate::{
    database::MapsetTagWrapper,
    error::BgGameError,
    util::{constants::OSU_BASE, MessageExt},
    Context, MessageBuilder,
};

use hashbrown::HashMap;
use parking_lot::{Mutex, RwLock};
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    time::{sleep, Duration},
};
use twilight_model::{
    gateway::payload::MessageCreate,
    id::{ChannelId, MessageId},
};

const GAME_LEN: Duration = Duration::from_secs(180);

pub struct GameWrapper {
    pub game: Arc<RwLock<Option<Game>>>,
    tx: Arc<Mutex<Sender<LoopResult>>>,
    rx: Option<Arc<Mutex<Receiver<LoopResult>>>>,
}

impl GameWrapper {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(5);

        Self {
            game: Arc::new(RwLock::new(None)),
            tx: Arc::new(Mutex::new(tx)),
            rx: Some(Arc::new(Mutex::new(rx))),
        }
    }

    pub async fn stop(&self) -> GameResult<()> {
        let tx = self.tx.lock();

        tx.send(LoopResult::Stop)
            .await
            .map_err(|_| BgGameError::StopToken)
    }

    pub async fn restart(&self) -> GameResult<()> {
        let tx = self.tx.lock();

        tx.send(LoopResult::Restart)
            .await
            .map_err(|_| BgGameError::RestartToken)
    }

    pub fn sub_image(&self) -> GameResult<Option<Vec<u8>>> {
        match self.game.read().as_ref() {
            Some(game) => Some(game.sub_image()).transpose(),
            None => Ok(None),
        }
    }

    pub fn hint(&self) -> GameResult<Option<String>> {
        match self.game.read().as_ref() {
            Some(game) => Ok(Some(game.hint())),
            None => Ok(None),
        }
    }

    pub fn start(&mut self, ctx: Arc<Context>, channel: ChannelId, mapsets: Vec<MapsetTagWrapper>) {
        let mut msg_stream = ctx
            .standby
            .wait_for_message_stream(channel, |event: &MessageCreate| !event.author.bot);

        let game_lock = Arc::clone(&self.game);

        let rx = match self.rx.take() {
            Some(rx) => rx,
            None => return warn!("No rx left for bg game"),
        };

        let mut previous_ids = VecDeque::with_capacity(50);
        let mut scores = HashMap::new();

        tokio::spawn(async move {
            loop {
                // Initialize game
                let (game, img) = Game::new(&ctx, &mapsets, &mut previous_ids).await;
                {
                    let mut arced_game = game_lock.write();
                    *arced_game = Some(game);
                }

                let builder = MessageBuilder::new()
                    .content("Here's the next one:")
                    .file("bg_img.png", &img);

                if let Err(why) = (MessageId(0), channel).create_message(&ctx, builder).await {
                    unwind_error!(warn, why, "Error while sending initial bg game msg: {}");
                }

                let rx_fut = async {
                    let mut rx = rx.lock();

                    rx.recv().await
                };

                let result = tokio::select! {
                    // Listen for stop or restart invokes
                    option = rx_fut => option.unwrap_or(LoopResult::Stop),
                    // Let the game run
                    result = game_loop(&mut msg_stream, &ctx, &game_lock, channel) => result,
                    // Timeout after 3 minutes
                    _ = sleep(GAME_LEN) => LoopResult::Stop,
                };

                // Process the result
                match result {
                    LoopResult::Restart => {
                        let game = game_lock.read();

                        // Send message
                        if let Some(game) = game.as_ref() {
                            let content = format!(
                                "Full background: {}beatmapsets/{}",
                                OSU_BASE, game.mapset_id
                            );

                            if let Err(why) = game.resolve(&ctx, channel, &content).await {
                                unwind_error!(
                                    warn,
                                    why,
                                    "Error while showing resolve for bg game restart: {}"
                                );
                            }
                        } else {
                            debug!("Trying to restart on None");
                        }
                    }
                    LoopResult::Stop => {
                        let game = game_lock.read();

                        // Send message
                        if let Some(game) = game.as_ref() {
                            let content = format!(
                                "Full background: {}beatmapsets/{}\n\
                                End of game, see you next time o/",
                                OSU_BASE, game.mapset_id
                            );

                            if let Err(why) = game.resolve(&ctx, channel, &content).await {
                                unwind_error!(
                                    warn,
                                    why,
                                    "Error while showing resolve for bg game stop: {}"
                                );
                            }
                        } else {
                            debug!("Trying to stop on None");
                        }

                        // Store score for winners
                        for (user, score) in scores {
                            if let Err(why) = ctx.psql().increment_bggame_score(user, score).await {
                                unwind_error!(
                                    error,
                                    why,
                                    "Error while incrementing bggame score: {}"
                                );
                            }
                        }

                        // Then quit
                        info!("Game finished in channel {}", channel);
                        break;
                    }
                    LoopResult::Winner(user_id) => {
                        if mapsets.len() >= 20 {
                            *scores.entry(user_id).or_insert(0) += 1;
                        }
                    }
                }
            }

            ctx.remove_game(channel);
        });
    }
}
