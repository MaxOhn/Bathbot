use super::{game_loop, Game, GameResult, LoopResult};
use crate::{
    database::MapsetTagWrapper,
    unwind_error,
    util::{constants::OSU_BASE, error::BgGameError},
    Context,
};

use std::collections::HashMap;
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex, RwLock,
    },
    time::{sleep, timeout, Duration},
};
use twilight_model::{gateway::payload::MessageCreate, id::ChannelId};

const TIMEOUT: Duration = Duration::from_secs(10);
const GAME_LEN: Duration = Duration::from_secs(180);

pub struct GameWrapper {
    pub game: Arc<RwLock<Option<Game>>>,
    tx: Arc<Mutex<Sender<LoopResult>>>,
    rx: Option<Arc<Mutex<Receiver<LoopResult>>>>,
}

impl GameWrapper {
    #[inline]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(5);

        Self {
            game: Arc::new(RwLock::new(None)),
            tx: Arc::new(Mutex::new(tx)),
            rx: Some(Arc::new(Mutex::new(rx))),
        }
    }

    #[inline]
    pub async fn stop(&self) -> GameResult<()> {
        let tx = self.tx.lock().await;

        tx.send(LoopResult::Stop)
            .await
            .map_err(|_| BgGameError::StopToken)
    }

    #[inline]
    pub async fn restart(&self) -> GameResult<()> {
        let tx = self.tx.lock().await;

        tx.send(LoopResult::Restart)
            .await
            .map_err(|_| BgGameError::RestartToken)
    }

    #[inline]
    pub async fn sub_image(&self) -> GameResult<Option<Vec<u8>>> {
        let game_option = timeout(TIMEOUT, self.game.read()).await?;

        match game_option.as_ref() {
            Some(game) => Some(game.sub_image().await).transpose(),
            None => Ok(None),
        }
    }

    #[inline]
    pub async fn hint(&self) -> GameResult<Option<String>> {
        let game_option = timeout(TIMEOUT, self.game.read()).await?;

        match game_option.as_ref() {
            Some(game) => Ok(Some(game.hint().await)),
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
            None => {
                warn!("No rx left for bg game");
                return;
            }
        };

        let mut previous_ids = VecDeque::with_capacity(50);
        let mut scores = HashMap::new();

        tokio::spawn(async move {
            loop {
                // Initialize game
                let (game, img) = Game::new(&ctx, &mapsets, &mut previous_ids).await;
                {
                    let mut arced_game = game_lock.write().await;
                    *arced_game = Some(game);
                }

                let msg_result = ctx
                    .http
                    .create_message(channel)
                    .content("Here's the next one:")
                    .unwrap()
                    .attachment("bg_img.png", img)
                    .await;

                if let Err(why) = msg_result {
                    unwind_error!(warn, why, "Error while sending initial bg game msg: {}");
                }

                let rx_fut = async {
                    let mut rx = rx.lock().await;
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
                        // Send message
                        let game_option = game_lock.read().await;

                        if let Some(game) = game_option.as_ref() {
                            let content = format!(
                                "Full background: {}beatmapsets/{}",
                                OSU_BASE, game.mapset_id
                            );

                            if let Err(why) = game.resolve(&ctx, channel, content).await {
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
                        // Send message
                        let game_option = game_lock.read().await;

                        if let Some(game) = game_option.as_ref() {
                            let content = format!(
                                "Full background: {}beatmapsets/{}\n\
                                End of game, see you next time o/",
                                OSU_BASE, game.mapset_id
                            );

                            if let Err(why) = game.resolve(&ctx, channel, content).await {
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
                        debug!("Game finished in channel {}", channel);

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
