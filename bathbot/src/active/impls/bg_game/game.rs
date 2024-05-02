use std::{collections::VecDeque, sync::RwLock};

use bathbot_model::Effects;
use bathbot_psql::model::games::MapsetTagsEntries;
use bathbot_util::{constants::OSU_BASE, CowUtils};
use eyre::{Result, WrapErr};
use image::{
    imageops::{self, colorops},
    GenericImageView,
};
use rosu_v2::model::GameMode;
use tokio::{fs, sync::RwLock as TokioRwLock};
use tokio_stream::StreamExt;
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};
use twilight_standby::future::WaitForMessageStream;

use super::{hints::Hints, img_reveal::ImageReveal, mapset::GameMapset, util};
use crate::{commands::fun::GameDifficulty, core::BotConfig, util::ChannelExt, Context};

pub struct Game {
    pub mapset: GameMapset,
    difficulty: f32,
    hints: RwLock<Hints>,
    reveal: RwLock<ImageReveal>,
}

impl Game {
    pub async fn new(
        entries: &MapsetTagsEntries,
        previous_ids: &mut VecDeque<i32>,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> (Self, Vec<u8>) {
        loop {
            match Game::new_(entries, previous_ids, effects, difficulty).await {
                Ok(game) => {
                    let sub_image_result = { game.reveal.read().unwrap().sub_image() };

                    match sub_image_result {
                        Ok(img) => return (game, img),
                        Err(err) => {
                            warn!(
                                mapset_id = game.mapset.mapset_id,
                                ?err,
                                "Failed to create initial bg game"
                            );
                        }
                    }
                }
                Err(err) => {
                    warn!(?err, "Error while creating bg game");
                }
            }
        }
    }

    async fn new_(
        entries: &MapsetTagsEntries,
        previous_ids: &mut VecDeque<i32>,
        effects: Effects,
        difficulty: GameDifficulty,
    ) -> Result<Self> {
        let mut path = BotConfig::get().paths.backgrounds.clone();

        match entries.mode {
            GameMode::Osu => path.push("osu"),
            GameMode::Mania => path.push("mania"),
            _ => bail!("background game not available for {}", entries.mode),
        }

        let mapset = util::get_random_mapset(entries, previous_ids);
        let mapset_id = mapset.mapset_id;
        debug!("Next BG mapset id: {mapset_id}");
        path.push(&mapset.image_filename);

        let img_fut = async {
            let bytes = fs::read(path)
                .await
                .wrap_err_with(|| format!("failed to read bg image for mapset {mapset_id}"))?;

            let mut img =
                image::load_from_memory(&bytes).wrap_err("failed to load image from memory")?;

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

        let (mapset_, img) = tokio::try_join!(GameMapset::new(mapset.mapset_id as u32), img_fut)?;

        Ok(Self {
            hints: RwLock::new(Hints::new(mapset_.title())),
            difficulty: difficulty.factor(),
            mapset: mapset_,
            reveal: RwLock::new(ImageReveal::new(img)),
        })
    }

    pub fn sub_image(&self) -> Result<Vec<u8>> {
        let mut reveal = self.reveal.write().unwrap();
        reveal.increase_radius();

        reveal.sub_image()
    }

    pub fn hint(&self) -> String {
        let mut hints = self.hints.write().unwrap();

        hints.get(self.mapset.title(), self.mapset.artist())
    }

    pub fn mapset_id(&self) -> u32 {
        self.mapset.mapset_id
    }

    fn check_msg_content(&self, content: &str) -> ContentResult {
        match self.mapset.matches_title(content, self.difficulty) {
            Some(true) => return ContentResult::Title(true),
            Some(false) => return ContentResult::Title(false),
            None => {}
        }

        if !self.hints.read().unwrap().artist_guessed {
            match self.mapset.matches_artist(content, self.difficulty) {
                Some(true) => return ContentResult::Artist(true),
                Some(false) => return ContentResult::Artist(false),
                None => {}
            }
        }

        ContentResult::None
    }
}

#[derive(Clone, Copy)]
pub enum LoopResult {
    Winner(Id<UserMarker>),
    Restart,
    Stop,
}

pub async fn game_loop(
    msg_stream: &mut WaitForMessageStream,
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
                    Mapset: {OSU_BASE}beatmapsets/{mapset_id}\n\
                    Full background: https://assets.ppy.sh/beatmaps/{mapset_id}/covers/raw.jpg",
                    if exact {
                        format!("Gratz {}, you guessed it", msg.author.name)
                    } else {
                        format!("You were close enough {}, gratz", msg.author.name)
                    },
                    mapset_id = game.mapset.mapset_id
                );

                // Send message
                if let Err(err) = channel.plain_message(&content).await {
                    warn!(?err, "Error while sending msg for winner");
                }

                return LoopResult::Winner(msg.author.id);
            }
            // Artist correct?
            ContentResult::Artist(exact) => {
                game.hints.write().unwrap().artist_guessed = true;

                let content = if exact {
                    format!(
                        "That's the correct artist `{}`, can you get the title too?",
                        msg.author.name
                    )
                } else {
                    format!(
                        "`{}` got the artist almost correct, \
                        it's actually `{}` but can you get the title?",
                        msg.author.name,
                        game.mapset.artist()
                    )
                };

                // Send message
                let msg_fut = Context::http()
                    .create_message(channel)
                    .content(&content)
                    .unwrap();

                if let Err(err) = msg_fut.await {
                    warn!(?err, "Error while sending msg for correct artist");
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
