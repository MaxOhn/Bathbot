use super::{util, GameResult, Hints, ImageReveal};
use crate::{
    database::MapsetTagWrapper,
    util::{
        constants::OSU_BASE, error::BgGameError, gestalt_pattern_matching, levenshtein_similarity,
        CowUtils,
    },
    BotResult, Context, CONFIG,
};

use futures::future::TryFutureExt;
use image::GenericImageView;
use rosu_v2::model::GameMode;
use std::{collections::VecDeque, sync::Arc};
use tokio::{fs, sync::RwLock};
use tokio_stream::StreamExt;
use twilight_model::id::ChannelId;
use twilight_standby::WaitForMessageStream;

pub struct Game {
    pub title: String,
    pub artist: String,
    pub mapset_id: u32,
    hints: Arc<RwLock<Hints>>,
    reveal: Arc<RwLock<ImageReveal>>,
}

impl Game {
    pub async fn new(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> (Self, Vec<u8>) {
        loop {
            match Game::_new(ctx, mapsets, previous_ids).await {
                Ok(game) => {
                    let sub_image_result = {
                        let reveal = game.reveal.read().await;

                        reveal.sub_image()
                    };

                    match sub_image_result {
                        Ok(img) => return (game, img),
                        Err(why) => unwind_error!(
                            warn,
                            why,
                            "Could not create initial bg image for id {}: {}",
                            game.mapset_id
                        ),
                    }
                }
                Err(why) => unwind_error!(warn, why, "Error creating bg game: {}"),
            }
        }
    }

    async fn _new(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> GameResult<Self> {
        let mut path = CONFIG.get().unwrap().bg_path.clone();

        match mapsets[0].mode {
            GameMode::STD => path.push("osu"),
            GameMode::MNA => path.push("mania"),
            _ => return Err(BgGameError::Mode(mapsets[0].mode)),
        }

        let mapset = util::get_random_mapset(mapsets, previous_ids).await;
        let mapset_id = mapset.mapset_id;
        debug!("Next BG mapset id: {}", mapset_id);
        path.push(&mapset.filename);

        let img_fut = fs::read(path)
            .map_err(|err| BgGameError::IO(err, mapset_id))
            .and_then(|img_vec| {
                async move { image::load_from_memory(&img_vec) }
                    .map_ok(|img| {
                        let (w, h) = img.dimensions();

                        // 800*600 (4:3)
                        if w * h > 480_000 {
                            img.thumbnail(800, 600)
                        } else {
                            img
                        }
                    })
                    .map_err(BgGameError::from)
            });

        let ((title, artist), img) =
            tokio::try_join!(util::get_title_artist(ctx, mapset.mapset_id), img_fut)?;

        Ok(Self {
            hints: Arc::new(RwLock::new(Hints::new(&title, mapset.tags))),
            title,
            artist,
            mapset_id: mapset.mapset_id,
            reveal: Arc::new(RwLock::new(ImageReveal::new(img))),
        })
    }

    #[inline]
    pub async fn sub_image(&self) -> GameResult<Vec<u8>> {
        let mut reveal = self.reveal.write().await;
        reveal.increase_radius();

        reveal.sub_image()
    }

    #[inline]
    pub async fn hint(&self) -> String {
        let mut hints = self.hints.write().await;

        hints.get(&self.title, &self.artist)
    }

    pub async fn resolve(
        &self,
        ctx: &Context,
        channel: ChannelId,
        content: String,
    ) -> BotResult<()> {
        let reveal_result = {
            let reveal = self.reveal.read().await;

            reveal.full()
        };

        match reveal_result {
            Ok(bytes) => {
                ctx.http
                    .create_message(channel)
                    .content(content)?
                    .attachment("bg_img.png", bytes)
                    .await?;
            }
            Err(why) => {
                unwind_error!(
                    warn,
                    why,
                    "Could not get full reveal of mapset id {}: {}",
                    self.mapset_id
                );

                ctx.http.create_message(channel).content(content)?.await?;
            }
        }

        Ok(())
    }

    async fn check_msg_content(&self, content: &str) -> ContentResult {
        // Guessed the title exactly?
        if content == self.title {
            return ContentResult::Title(true);
        }

        // First check the title through levenshtein distance.
        let similarity = levenshtein_similarity(content, &self.title);

        // Then through longest common substrings (generally more lenient than levenshtein)
        if similarity > 0.5 || gestalt_pattern_matching(content, &self.title) > 0.5 {
            return ContentResult::Title(false);
        }

        if !self.hints.read().await.artist_guessed {
            // Guessed the artist exactly?
            if content == self.artist {
                return ContentResult::Artist(true);
            // Dissimilar enough from the title but similar enough to the artist?
            } else if similarity < 0.3 && levenshtein_similarity(content, &self.artist) > 0.5 {
                return ContentResult::Artist(false);
            }
        }

        ContentResult::None
    }
}

#[derive(Clone, Copy)]
pub enum LoopResult {
    Winner(u64),
    Restart,
    Stop,
}

pub async fn game_loop(
    msg_stream: &mut WaitForMessageStream,
    ctx: &Context,
    game_lock: &RwLock<Option<Game>>,
    channel: ChannelId,
) -> LoopResult {
    // Collect and evaluate messages
    while let Some(msg) = msg_stream.next().await {
        let game = game_lock.read().await;

        if let Some(game) = game.as_ref() {
            let content = msg.content.cow_to_ascii_lowercase();

            match game.check_msg_content(content.as_ref()).await {
                // Title correct?
                ContentResult::Title(exact) => {
                    let content = format!(
                        "{} \\:)\nMapset: {}beatmapsets/{}",
                        if exact {
                            format!("Gratz {}, you guessed it", msg.author.name)
                        } else {
                            format!("You were close enough {}, gratz", msg.author.name)
                        },
                        OSU_BASE,
                        game.mapset_id
                    );

                    // Send message
                    if let Err(why) = game.resolve(ctx, channel, content).await {
                        unwind_error!(warn, why, "Error while sending msg for winner: {}");
                    }

                    return LoopResult::Winner(msg.author.id.0);
                }
                // Artist correct?
                ContentResult::Artist(exact) => {
                    {
                        let mut hints = game.hints.write().await;
                        hints.artist_guessed = true;
                    }

                    let content = if exact {
                        format!(
                            "That's the correct artist `{}`, can you get the title too?",
                            msg.author.name
                        )
                    } else {
                        format!(
                            "`{}` got the artist almost correct, \
                            it's actually `{}` but can you get the title?",
                            msg.author.name, game.artist
                        )
                    };

                    // Send message
                    let msg_fut = ctx.http.create_message(channel).content(content).unwrap();

                    if let Err(why) = msg_fut.await {
                        unwind_error!(warn, why, "Error while sending msg for correct artist: {}");
                    }
                }
                ContentResult::None => {}
            }
        } else {
            return LoopResult::Stop;
        }
    }

    LoopResult::Stop
}

// bool to tell whether its an exact match
enum ContentResult {
    Title(bool),
    Artist(bool),
    None,
}
