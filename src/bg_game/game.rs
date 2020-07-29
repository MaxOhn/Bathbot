use super::{util, GameResult, Hints, ImageReveal};
use crate::{
    database::MapsetTagWrapper,
    util::{constants::OSU_BASE, error::BgGameError},
    BotResult, Context,
};

use image::GenericImageView;
use rosu::models::GameMode;
use std::{collections::VecDeque, env, fmt::Write, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    stream::StreamExt,
    sync::{
        watch::{channel, Receiver, Sender},
        RwLock,
    },
    time::{delay_for, timeout, Duration},
};
use twilight::model::{gateway::payload::MessageCreate, id::ChannelId};
use twilight::standby::WaitForMessageStream;

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Game {
    pub title: String,
    pub artist: String,
    pub mapset_id: u32,
    hints: Hints,
    reveal: ImageReveal,
}

impl Game {
    pub async fn new(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> (Self, Vec<u8>) {
        loop {
            match Game::_new(ctx, mapsets, previous_ids).await {
                Ok(game) => match game.reveal.sub_image() {
                    Ok(img) => return (game, img),
                    Err(why) => warn!(
                        "Could not create initial bg image for id {}: {}",
                        game.mapset_id, why
                    ),
                },
                Err(why) => warn!("Error creating bg game: {}", why),
            }
        }
    }

    async fn _new(
        ctx: &Context,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> GameResult<Self> {
        let mut path = ctx.config.bg_path.clone();
        match mapsets[0].mode {
            GameMode::STD => path.push("osu"),
            GameMode::MNA => path.push("mania"),
            _ => return Err(BgGameError::Mode(mapsets[0].mode)),
        }
        let mapset = util::get_random_mapset(mapsets, previous_ids).await;
        debug!("Next BG mapset id: {}", mapset.mapset_id);
        let (title, artist) = util::get_title_artist(ctx, mapset.mapset_id).await?;
        let filename = format!("{}.{}", mapset.mapset_id, mapset.filetype);
        path.push(filename);
        let img_vec = fs::read(path).await?;
        let mut img = image::load_from_memory(&img_vec)?;
        let (w, h) = img.dimensions();
        // 800*600 (4:3)
        if w * h > 480_000 {
            img = img.thumbnail(800, 600);
        }
        Ok(Self {
            hints: Hints::new(&title, mapset.tags),
            title,
            artist,
            mapset_id: mapset.mapset_id,
            reveal: ImageReveal::new(img),
        })
    }

    pub fn sub_image(&mut self) -> GameResult<Vec<u8>> {
        self.reveal.increase_radius();
        self.reveal.sub_image()
    }

    pub fn hint(&mut self) -> String {
        self.hints.get(&self.title, &self.artist)
    }

    pub async fn resolve(
        &self,
        ctx: &Context,
        channel: ChannelId,
        content: String,
    ) -> BotResult<()> {
        match self.reveal.full() {
            Ok(bytes) => {
                ctx.http
                    .create_message(channel)
                    .content(content)?
                    .attachment("bg_img.png", bytes)
                    .await?;
            }
            Err(why) => {
                warn!(
                    "Could not get full reveal of mapset id {}: {}",
                    self.mapset_id, why
                );
                ctx.http.create_message(channel).content(content)?.await?;
            }
        }
        Ok(())
    }

    fn check_msg_content(&self, content: &str) -> ContentResult {
        // Guessed the title exactly?
        if content == self.title {
            return ContentResult::Title(true);
        }
        // Guessed sufficiently many words of the title?
        if self.title.contains(' ') {
            let mut same_word_len = 0;
            for title_word in self.title.split(' ') {
                for content_word in content.split(' ') {
                    if title_word == content_word {
                        same_word_len += title_word.len();
                        if same_word_len > 8 {
                            return ContentResult::Title(false);
                        }
                    }
                }
            }
        }
        // Similar enough to the title?
        let similarity = util::similarity(content, &self.title);
        if similarity > 0.5 {
            return ContentResult::Title(false);
        }
        if !self.hints.artist_guessed {
            // Guessed the artist exactly?
            if content == self.artist {
                return ContentResult::Artist(true);
            // Similar enough to the artist?
            } else if similarity < 0.3 && util::similarity(content, &self.artist) > 0.5 {
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
        let mut game = game_lock.write().await;
        if game.is_none() {
            return LoopResult::Stop;
        }
        let mut game = game.as_mut().unwrap();
        match game.check_msg_content(&msg.content) {
            // Title correct?
            ContentResult::Title(exact) => {
                let content = format!(
                    "{} \\:)\nMapset: {}/beatmapsets/{}",
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
                    warn!("Error while sending msg for winner: {}", why);
                }
                return LoopResult::Winner(msg.author.id.0);
            }
            // Artist correct?
            ContentResult::Artist(exact) => {
                game.hints.artist_guessed = true;
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
                    warn!("Error while sending msg for correct artist: {}", why);
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
