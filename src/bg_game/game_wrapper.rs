use super::{game_loop, Game, GameResult, LoopResult};
use crate::{
    database::MapsetTagWrapper,
    error::BgGameError,
    util::{constants::OSU_BASE, MessageExt},
    Context, MessageBuilder,
};

use eyre::Report;
use hashbrown::HashMap;
use parking_lot::RwLock;
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::broadcast::{self, Receiver, Sender},
    time::{sleep, Duration},
};
use twilight_model::{
    gateway::payload::incoming::MessageCreate,
    id::{ChannelId, MessageId},
};

const GAME_LEN: Duration = Duration::from_secs(180);

pub struct GameWrapper {
    pub game: Arc<RwLock<Option<Game>>>,
    tx: Sender<LoopResult>,
    _rx: Receiver<LoopResult>,
}

impl GameWrapper {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(10);

        Self {
            game: Arc::new(RwLock::new(None)),
            tx,
            _rx,
        }
    }

    pub fn stop(&self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Stop)
            .map(|_| ())
            .map_err(|_| BgGameError::StopToken)
    }

    pub fn restart(&self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Restart)
            .map(|_| ())
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
        let rx = self.tx.subscribe();

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

                let tmp_msg = (MessageId::new(1).unwrap(), channel);
                let create_fut = tmp_msg.create_message(&ctx, builder);

                if let Err(why) = create_fut.await {
                    let report =
                        Report::new(why).wrap_err("error while sending initial bg game msg");
                    warn!("{:?}", report);
                }

                let rx_fut = rx.recv();

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
                                let report = Report::new(why)
                                    .wrap_err("error while showing resolve for bg game restart");
                                warn!("{:?}", report);
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
                                let report = Report::new(why)
                                    .wrap_err("error while showing resolve for bg game stop");
                                warn!("{:?}", report);
                            }
                        } else {
                            debug!("Trying to stop on None");
                        }

                        // Store score for winners
                        for (user, score) in scores {
                            if let Err(why) = ctx.psql().increment_bggame_score(user, score).await {
                                let report = Report::new(why)
                                    .wrap_err("error while incrementing bg game score");
                                warn!("{:?}", report);
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
