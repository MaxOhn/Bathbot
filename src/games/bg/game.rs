use std::{collections::VecDeque, sync::Arc};

use eyre::Report;
use futures::future::TryFutureExt;
use image::{
    imageops::{self, colorops},
    GenericImageView,
};
use parking_lot::RwLock;
use rosu_v2::model::GameMode;
use tokio::{fs, sync::RwLock as TokioRwLock};
use tokio_stream::StreamExt;
use twilight_model::id::{marker::ChannelMarker, Id};
use twilight_standby::future::WaitForMessageStream;

use crate::{
    commands::fun::GameDifficulty,
    database::MapsetTagWrapper,
    error::BgGameError,
    games::bg::{hints::Hints, img_reveal::ImageReveal},
    util::{
        constants::OSU_BASE, gestalt_pattern_matching, levenshtein_similarity, ChannelExt, CowUtils,
    },
    Context, CONFIG,
};

use super::{util, Effects, GameResult};

pub struct Game {
    pub title: String,
    pub artist: String,
    pub mapset_id: u32,
    difficulty: f32,
    hints: Arc<RwLock<Hints>>,
    reveal: Arc<RwLock<ImageReveal>>,
}

impl Game {
    pub async fn new(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> (Self, Vec<u8>) {
        loop {
            match Game::new_(ctx, mapsets, previous_ids, effects, difficulty).await {
                Ok(game) => {
                    let sub_image_result = { game.reveal.read().sub_image() };

                    match sub_image_result {
                        Ok(img) => return (game, img),
                        Err(why) => {
                            let wrap = format!(
                                "failed to create initial bg image for id {}",
                                game.mapset_id
                            );
                            let report = Report::new(why).wrap_err(wrap);
                            warn!("{report:?}");
                        }
                    }
                }
                Err(why) => {
                    let report = Report::new(why).wrap_err("error while creating bg game");
                    warn!("{report:?}");
                }
            }
        }
    }

    async fn new_(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> GameResult<Self> {
        let mut path = CONFIG.get().unwrap().paths.backgrounds.clone();

        match mapsets[0].mode {
            GameMode::STD => path.push("osu"),
            GameMode::MNA => path.push("mania"),
            _ => return Err(BgGameError::Mode(mapsets[0].mode)),
        }

        let mapset = util::get_random_mapset(mapsets, previous_ids).await;
        let mapset_id = mapset.mapset_id;
        debug!("Next BG mapset id: {mapset_id}");
        path.push(&mapset.filename);

        let img_fut = async {
            let bytes = fs::read(path)
                .map_err(|source| BgGameError::Io { source, mapset_id })
                .await?;

            let mut img = image::load_from_memory(&bytes).map_err(BgGameError::Image)?;

            let (w, h) = img.dimensions();

            // 800*600 (4:3)
            if w * h > 480_000 {
                img = img.thumbnail(800, 600);
            }

            if effects.contains(Effects::Invert) {
                img.invert();
            }

            if effects.contains(Effects::Contrast) {
                colorops::contrast_in_place(&mut img, 18.0);
            }

            if effects.contains(Effects::FlipHorizontal) {
                imageops::flip_horizontal_in_place(&mut img);
            }

            if effects.contains(Effects::FlipVertical) {
                imageops::flip_vertical_in_place(&mut img);
            }

            if effects.contains(Effects::Grayscale) {
                img = img.grayscale();
            }

            if effects.contains(Effects::Blur) {
                img = img.blur(4.0);
            }

            Ok(img)
        };

        let ((title, artist), img) =
            tokio::try_join!(util::get_title_artist(ctx, mapset.mapset_id), img_fut)?;

        Ok(Self {
            hints: Arc::new(RwLock::new(Hints::new(&title, mapset.tags))),
            title,
            artist,
            difficulty: difficulty.factor(),
            mapset_id: mapset.mapset_id,
            reveal: Arc::new(RwLock::new(ImageReveal::new(img))),
        })
    }

    pub fn sub_image(&self) -> GameResult<Vec<u8>> {
        let mut reveal = self.reveal.write();
        reveal.increase_radius();

        reveal.sub_image()
    }

    pub fn hint(&self) -> String {
        let mut hints = self.hints.write();

        hints.get(&self.title, &self.artist)
    }

    fn check_msg_content(&self, content: &str) -> ContentResult {
        // Guessed the title exactly?
        if content == self.title {
            return ContentResult::Title(true);
        }

        // First check the title through levenshtein distance.
        let similarity = levenshtein_similarity(content, &self.title);

        // Then through longest common substrings (generally more lenient than levenshtein)
        if similarity > self.difficulty
            || gestalt_pattern_matching(content, &self.title) > self.difficulty + 0.1
        {
            return ContentResult::Title(false);
        }

        if !self.hints.read().artist_guessed {
            // Guessed the artist exactly?
            if content == self.artist {
                return ContentResult::Artist(true);
            // Dissimilar enough from the title but similar enough to the artist?
            } else if similarity < 0.3
                && levenshtein_similarity(content, &self.artist) > self.difficulty
            {
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
    game_locked: &TokioRwLock<Game>,
    channel: Id<ChannelMarker>,
) -> LoopResult {
    // Collect and evaluate messages
    while let Some(msg) = msg_stream.next().await {
        let game = game_locked.read().await;
        let content = msg.content.cow_to_ascii_lowercase();

        match game.check_msg_content(content.as_ref()) {
            // Title correct?
            ContentResult::Title(exact) => {
                let content = format!(
                    "{} \\:)\n\
                    Mapset: {}beatmapsets/{mapset_id}\n\
                    Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg",
                    if exact {
                        format!("Gratz {}, you guessed it", msg.author.name)
                    } else {
                        format!("You were close enough {}, gratz", msg.author.name)
                    },
                    OSU_BASE,
                    mapset_id = game.mapset_id
                );

                // Send message
                if let Err(why) = channel.plain_message(ctx, &content).await {
                    let report = Report::new(why).wrap_err("error while sending msg for winner");
                    warn!("{report:?}");
                }

                return LoopResult::Winner(msg.author.id.get());
            }
            // Artist correct?
            ContentResult::Artist(exact) => {
                {
                    let mut hints = game.hints.write();
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
                let msg_fut = ctx.http.create_message(channel).content(&content).unwrap();

                if let Err(why) = msg_fut.exec().await {
                    let report =
                        Report::new(why).wrap_err("error while sending msg for correct artist");
                    warn!("{report:?}");
                }
            }
            ContentResult::None => {}
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
