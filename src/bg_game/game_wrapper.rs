use std::{collections::VecDeque, sync::Arc};
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    time::{sleep, Duration},
};

use eyre::Report;
use hashbrown::HashMap;
use tokio::sync::RwLock;
use twilight_model::{
    gateway::payload::incoming::MessageCreate,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use crate::{
    commands::fun::{Effects, GameDifficulty},
    database::MapsetTagWrapper,
    util::{constants::OSU_BASE, MessageExt},
    Context, MessageBuilder,
};

use super::{game_loop, BgGameError, Game, GameResult, LoopResult};

const GAME_LEN: Duration = Duration::from_secs(180);

pub struct GameWrapper {
    game: Arc<RwLock<Game>>,
    tx: UnboundedSender<LoopResult>,
}

impl GameWrapper {
    pub async fn new(
        ctx: Arc<Context>,
        channel: Id<ChannelMarker>,
        mapsets: Vec<MapsetTagWrapper>,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut msg_stream = ctx
            .standby
            .wait_for_message_stream(channel, |event: &MessageCreate| !event.author.bot);

        let mut previous_ids = VecDeque::with_capacity(50);
        let mut scores = HashMap::new();

        // Initialize game
        let (game, mut img) =
            Game::new(&ctx, &mapsets, &mut previous_ids, effects, difficulty).await;
        let game = Arc::new(RwLock::new(game));
        let game_clone = Arc::clone(&game);

        tokio::spawn(async move {
            loop {
                let builder = MessageBuilder::new()
                    .content("Here's the next one:")
                    .file("bg_img.png", &img);

                let tmp_msg = (Id::<MessageMarker>::new(1), channel);
                let create_fut = tmp_msg.create_message(&ctx, builder);

                if let Err(why) = create_fut.await {
                    let report =
                        Report::new(why).wrap_err("error while sending initial bg game msg");
                    warn!("{report:?}");
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
                        let game = game_clone.read().await;

                        // Send message
                        let content = format!(
                            "Mapset: {OSU_BASE}beatmapsets/{mapset_id}\n\
                            Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg",
                            mapset_id = game.mapset_id
                        );

                        if let Err(why) = super::send_msg(&ctx, channel, &content).await {
                            let report = Report::new(why)
                                .wrap_err("error while showing resolve for bg game restart");
                            warn!("{report:?}");
                        }
                    }
                    LoopResult::Stop => {
                        let game = game_clone.read().await;

                        // Send message
                        let content = format!(
                            "Mapset: {OSU_BASE}beatmapsets/{mapset_id}\n\
                            Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg\n\
                            End of game, see you next time o/",
                            mapset_id = game.mapset_id
                        );

                        if let Err(why) = super::send_msg(&ctx, channel, &content).await {
                            let report = Report::new(why)
                                .wrap_err("error while showing resolve for bg game stop");
                            warn!("{report:?}");
                        }

                        // Store score for winners
                        for (user, score) in scores {
                            if let Err(why) = ctx.psql().increment_bggame_score(user, score).await {
                                let report = Report::new(why)
                                    .wrap_err("error while incrementing bg game score");
                                warn!("{report:?}");
                            }
                        }

                        // Then quit
                        info!("Game finished in channel {channel}");
                        break;
                    }
                    LoopResult::Winner(user_id) => {
                        if mapsets.len() >= 20 {
                            *scores.entry(user_id).or_insert(0) += 1;
                        }
                    }
                }

                // Initialize next game
                let (game, img_) =
                    Game::new(&ctx, &mapsets, &mut previous_ids, effects, difficulty).await;
                img = img_;
                let mut unlocked_game = game_clone.write().await;
                *unlocked_game = game;
            }

            ctx.bg_games().remove(&channel);
        });

        Self { game, tx }
    }

    pub fn stop(&self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Stop)
            .map_err(|_| BgGameError::StopToken)
    }

    pub fn restart(&self) -> GameResult<()> {
        self.tx
            .send(LoopResult::Restart)
            .map_err(|_| BgGameError::RestartToken)
    }

    pub async fn sub_image(&self) -> GameResult<Vec<u8>> {
        self.game.read().await.sub_image()
    }

    pub async fn hint(&self) -> String {
        self.game.read().await.hint()
    }
}
