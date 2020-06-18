use super::{util, Hints, ImageReveal};
use crate::{database::MapsetTagWrapper, util::globals::HOMEPAGE, BgGames, MySQL};

use failure::Error;
use image::{imageops::FilterType, GenericImageView};
use rosu::models::GameMode;
use serenity::{
    collector::{MessageCollector, MessageCollectorBuilder},
    framework::standard::CommandResult,
    http::client::Http,
    model::id::ChannelId,
    prelude::{Context, RwLock, TypeMap},
};
use std::{collections::VecDeque, env, fmt::Write, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    stream::StreamExt,
    sync::watch::{channel, Receiver, Sender},
    time,
};

// Everything in here is coded horribly :(
pub struct BackGroundGame {
    pub game: Arc<RwLock<GameData>>,
    tx: Sender<LoopResult>,
    rx: Receiver<LoopResult>,
}

impl BackGroundGame {
    pub async fn new() -> Self {
        let (tx, mut rx) = channel(LoopResult::Restart);
        let _ = rx.recv().await; // zzz
        Self {
            game: Arc::new(RwLock::new(GameData::default())),
            tx,
            rx,
        }
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        Ok(self
            .tx
            .broadcast(LoopResult::Stop)
            .map_err(|_| format_err!("Could not send stop message"))?)
    }

    pub fn restart(&mut self) -> Result<(), Error> {
        Ok(self
            .tx
            .broadcast(LoopResult::Restart)
            .map_err(|_| format_err!("Could not send restart message"))?)
    }

    pub async fn sub_image(&self) -> Result<Vec<u8>, Error> {
        let mut game = self.game.write().await;
        game.sub_image()
    }

    pub async fn hint(&self) -> String {
        let mut game = self.game.write().await;
        game.hint()
    }

    pub async fn start(&self, ctx: &Context, channel: ChannelId, mapsets: Vec<MapsetTagWrapper>) {
        let mut collector = MessageCollectorBuilder::new(ctx)
            .channel_id(channel)
            .filter(|msg| !msg.author.bot)
            .await;
        let game_lock = Arc::clone(&self.game);
        let mut rx = self.rx.clone();
        let data = Arc::clone(&ctx.data);
        let http = Arc::clone(&ctx.http);
        tokio::spawn(async move {
            let mut previous_ids = VecDeque::with_capacity(100);
            loop {
                // Initialize game
                let img = {
                    let mut game = game_lock.write().await;
                    game.restart_with_img(Arc::clone(&data), &mapsets, &mut previous_ids)
                        .await
                };
                let _ = channel
                    .send_message(&http, |m| {
                        let bytes: &[u8] = &img;
                        m.content("Here's the next one:")
                            .add_file((bytes, "bg_img.png"))
                    })
                    .await;

                let result = tokio::select! {
                    // Listen for stop or restart invokes
                    option = rx.recv() => option.unwrap_or_else(|| LoopResult::Stop),
                    // Let the game run
                    result = game_loop(&mut collector, &http, &game_lock, channel) => result,
                    // Timeout after 3 minutes
                    _ = time::delay_for(time::Duration::from_secs(180)) => LoopResult::Stop,
                };

                // Process the result
                match result {
                    LoopResult::Restart => {
                        // Send message
                        let game = game_lock.read().await;
                        let content = format!(
                            "Full background: {}beatmapsets/{}",
                            HOMEPAGE, game.mapset_id
                        );
                        let _ = game.resolve(&http, channel, content).await;
                    }
                    LoopResult::Stop => {
                        // Send message
                        let game = game_lock.read().await;
                        let content = format!(
                            "Full background: {}beatmapsets/{}\n\
                            End of game, see you next time o/",
                            HOMEPAGE, game.mapset_id
                        );
                        let _ = game.resolve(&http, channel, content).await;
                        // Then quit
                        game.discord_data
                            .as_ref()
                            .unwrap()
                            .write()
                            .await
                            .get_mut::<BgGames>()
                            .unwrap()
                            .remove(&channel);
                        collector.stop();
                        debug!("Game finished");
                        break;
                    }
                    LoopResult::Winner(user_id) => {
                        if mapsets.len() >= 10 {
                            let data = data.read().await;
                            let mysql = data.get::<MySQL>().unwrap();
                            if let Err(why) = mysql.increment_bggame_score(user_id) {
                                error!("Error while incrementing bggame score: {}", why);
                            }
                        }
                    }
                }
            }
        });
    }
}

#[derive(Clone, Copy)]
enum LoopResult {
    Winner(u64),
    Restart,
    Stop,
}

async fn game_loop(
    collector: &mut MessageCollector,
    http: &Http,
    game_lock: &RwLock<GameData>,
    channel: ChannelId,
) -> LoopResult {
    // Collect and evaluate messages
    while let Some(msg) = collector.next().await {
        let mut game = game_lock.write().await;
        let content_result = check_msg_content(&msg.content, &game);
        match content_result {
            // Title correct?
            ContentResult::Title { exact } => {
                let mut content = if exact {
                    format!("Gratz {}, you guessed it", msg.author.name)
                } else {
                    format!("You were close enough {}, gratz", msg.author.name)
                };
                let _ = write!(
                    content,
                    " \\:)\nMapset: {}beatmapsets/{}",
                    HOMEPAGE, game.mapset_id
                );
                // Send message
                let _ = game.resolve(&http, channel, content).await;
                return LoopResult::Winner(msg.author.id.0);
            }
            // Artist correct?
            ContentResult::Artist { exact } => {
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
                let _ = channel.send_message(http, |m| m.content(content)).await;
            }
            ContentResult::None => {}
        }
    }
    LoopResult::Stop
}

enum ContentResult {
    Title { exact: bool },
    Artist { exact: bool },
    None,
}

fn check_msg_content(content: &str, game: &GameData) -> ContentResult {
    // Guessed the title exactly?
    if content == game.title {
        return ContentResult::Title { exact: true };
    }
    // Guessed sufficiently many words of the title?
    if game.title.contains(&" ") {
        let mut same_word_len = 0;
        for title_word in game.title.split(' ') {
            for content_word in content.split(' ') {
                if title_word == content_word {
                    same_word_len += title_word.len();
                    if same_word_len > 8 {
                        return ContentResult::Title { exact: false };
                    }
                }
            }
        }
    }
    // Similar enough to the title?
    let similarity = util::similarity(content, &game.title);
    if similarity > 0.5 {
        return ContentResult::Title { exact: false };
    }
    if !game.hints.artist_guessed {
        // Guessed the artist exactly?
        if content == game.artist {
            return ContentResult::Artist { exact: true };
        // Similar enough to the artist?
        } else if similarity < 0.3 && util::similarity(content, &game.artist) > 0.5 {
            return ContentResult::Artist { exact: false };
        }
    }
    ContentResult::None
}

#[derive(Default)]
pub struct GameData {
    pub title: String,
    pub artist: String,
    pub mapset_id: u32,
    discord_data: Option<Arc<RwLock<TypeMap>>>,
    hints: Hints,
    reveal: ImageReveal,
}

impl GameData {
    async fn restart(
        &mut self,
        data: Arc<RwLock<TypeMap>>,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> Result<(), Error> {
        let mut path = PathBuf::from(env::var("BG_PATH")?);
        match mapsets[0].mode {
            GameMode::STD => path.push("osu"),
            GameMode::MNA => path.push("mania"),
            GameMode::TKO | GameMode::CTB => panic!("TKO and CTB not yet supported as bg game"),
        }
        let mapset = util::get_random_mapset(mapsets, previous_ids).await?;
        debug!("Next BG mapset id: {}", mapset.mapset_id);
        let (title, artist) = util::get_title_artist(mapset.mapset_id, &data).await?;
        let filename = format!("{}.{}", mapset.mapset_id, mapset.filetype);
        path.push(filename);
        let img_vec = fs::read(path).await?;
        let mut img = image::load_from_memory(&img_vec)?;
        let (w, h) = img.dimensions();
        // 800*600 (4:3)
        if w * h > 480_000 {
            img = img.resize(800, 600, FilterType::Lanczos3);
        }
        self.hints = Hints::new(&title);
        self.title = title;
        self.artist = artist;
        self.mapset_id = mapset.mapset_id;
        self.reveal = ImageReveal::new(img);
        self.discord_data = Some(data);
        Ok(())
    }

    pub async fn restart_with_img(
        &mut self,
        data: Arc<RwLock<TypeMap>>,
        mapsets: &[MapsetTagWrapper],
        previous_ids: &mut VecDeque<u32>,
    ) -> Vec<u8> {
        loop {
            match self.restart(Arc::clone(&data), mapsets, previous_ids).await {
                Ok(_) => match self.reveal.sub_image() {
                    Ok(img) => return img,
                    Err(why) => warn!(
                        "Could not create initial bg image for id {}: {}",
                        self.mapset_id, why
                    ),
                },
                Err(why) => warn!("Error creating bg game: {}", why),
            }
        }
    }

    pub fn sub_image(&mut self) -> Result<Vec<u8>, Error> {
        self.reveal.increase_radius();
        self.reveal.sub_image()
    }

    pub fn hint(&mut self) -> String {
        self.hints.get(&self.title, &self.artist)
    }

    pub async fn resolve(&self, http: &Http, channel: ChannelId, content: String) -> CommandResult {
        match self.reveal.full() {
            Ok(bytes) => {
                channel
                    .send_message(http, |m| {
                        m.content(content)
                            .add_file((bytes.as_slice(), "bg_img.png"))
                    })
                    .await?;
            }
            Err(why) => {
                warn!(
                    "Could not get full reveal of mapset id {}: {}",
                    self.mapset_id, why
                );
                channel.send_message(http, |m| m.content(content)).await?;
            }
        }
        Ok(())
    }
}
