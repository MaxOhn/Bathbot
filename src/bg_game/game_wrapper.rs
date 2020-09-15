use super::{game_loop, Game, GameResult, LoopResult};
use crate::{
    database::MapsetTagWrapper,
    util::{constants::OSU_BASE, error::BgGameError},
    Context,
};

use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
    time::{delay_for, timeout, Duration},
};
use twilight_model::{gateway::payload::MessageCreate, id::ChannelId};

const TIMEOUT: Duration = Duration::from_secs(10);
const GAME_LEN: Duration = Duration::from_secs(180);

pub struct GameWrapper {
    pub game: Arc<RwLock<Option<Game>>>,
    tx: Sender<LoopResult>,
    rx: Option<Receiver<LoopResult>>,
}

impl GameWrapper {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(5);
        Self {
            game: Arc::new(RwLock::new(None)),
            tx,
            rx: Some(rx),
        }
    }

    pub async fn stop(&mut self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Stop)
            .await
            .map_err(|_| BgGameError::Stop)
    }

    pub async fn restart(&mut self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Restart)
            .await
            .map_err(|_| BgGameError::Restart)
    }

    pub async fn sub_image(&mut self) -> GameResult<Option<Vec<u8>>> {
        timeout(TIMEOUT, self.game.write())
            .await?
            .as_mut()
            .map_or_else(|| Ok(None), |game| Some(game.sub_image()).transpose())
    }

    pub async fn hint(&self) -> GameResult<Option<String>> {
        let hint = timeout(TIMEOUT, self.game.write())
            .await?
            .as_mut()
            .map(|game| game.hint());
        Ok(hint)
    }

    pub fn start(&mut self, ctx: Arc<Context>, channel: ChannelId, mapsets: Vec<MapsetTagWrapper>) {
        let mut msg_stream = ctx
            .standby
            .wait_for_message_stream(channel, |event: &MessageCreate| !event.author.bot);
        let game_lock = self.game.clone();
        let mut rx = self.rx.take().unwrap();
        let mut previous_ids = VecDeque::with_capacity(50);
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
                    warn!("Error while sending initial bg game msg: {}", why);
                }

                let result = tokio::select! {
                    // Listen for stop or restart invokes
                    option = rx.recv() => option.unwrap_or(LoopResult::Stop),
                    // Let the game run
                    result = game_loop(&mut msg_stream, &ctx, &game_lock, channel) => result,
                    // Timeout after 3 minutes
                    _ = delay_for(GAME_LEN) => LoopResult::Stop,
                };

                // Process the result
                match result {
                    LoopResult::Restart => {
                        // Send message
                        let game_option = game_lock.read().await;
                        if game_option.is_some() {
                            let game = game_option.as_ref().unwrap();
                            let content = format!(
                                "Full background: {}beatmapsets/{}",
                                OSU_BASE, game.mapset_id
                            );
                            if let Err(why) = game.resolve(&ctx, channel, content).await {
                                warn!("Error while showing resolve for bg game restart: {}", why);
                            }
                        } else {
                            debug!("Trying to restart on None");
                        }
                    }
                    LoopResult::Stop => {
                        // Send message
                        let game_option = game_lock.read().await;
                        if game_option.is_some() {
                            let game = game_option.as_ref().unwrap();
                            let content = format!(
                                "Full background: {}beatmapsets/{}\n\
                                End of game, see you next time o/",
                                OSU_BASE, game.mapset_id
                            );
                            if let Err(why) = game.resolve(&ctx, channel, content).await {
                                warn!("Error while showing resolve for bg game stop: {}", why);
                            }
                        } else {
                            debug!("Trying to stop on None");
                        }
                        // Then quit
                        debug!("Game finished in channel {}", channel);
                        break;
                    }
                    LoopResult::Winner(user_id) => {
                        if mapsets.len() >= 20 {
                            if let Err(why) = ctx.psql().increment_bggame_score(user_id).await {
                                error!("Error while incrementing bggame score: {}", why);
                            }
                        }
                    }
                }
            }
            ctx.remove_game(channel);
        });
    }
}
