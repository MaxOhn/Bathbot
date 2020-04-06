#![allow(unused_imports)]

use super::{util, Hints, ImageReveal};
use crate::{DispatchEvent, Error, MySQL};

use image::{imageops::FilterType, GenericImageView, ImageFormat};
use serenity::{
    cache::CacheRwLock,
    collector::MessageCollector,
    framework::standard::CommandResult,
    http::client::Http,
    model::id::{ChannelId, UserId},
    prelude::{Context, RwLock, ShareMap},
};
use std::{
    collections::VecDeque, env, fmt::Write, fs, path::PathBuf, str::FromStr, sync::Arc,
    time::Duration,
};
use tokio::task::{self, JoinHandle};

pub struct BackGroundGame {
    previous_ids: VecDeque<u32>,
    channel: ChannelId,
    osu_std: bool,
    collector: MessageCollector,
    game: GameData,
    http: Arc<Http>,
    data: Arc<RwLock<ShareMap>>,
}

impl BackGroundGame {
    pub async fn new(
        ctx: &Context,
        collector: MessageCollector,
        channel: ChannelId,
        osu_std: bool,
    ) -> Result<Self, Error> {
        let mut previous_ids = VecDeque::with_capacity(10);
        let http = Arc::clone(&ctx.http);
        let data = Arc::clone(&ctx.data);
        let game = GameData::new(Arc::clone(&data), &mut previous_ids, osu_std).await?;
        Ok(Self {
            previous_ids,
            channel,
            osu_std,
            game,
            http,
            data,
        })
    }

    async fn restart(&mut self) -> CommandResult {
        // TODO
        Ok(())
    }

    async fn run(collector: MessageCollector, data: Arc<RwLock<ShareMap>>) {
        task::spawn(async {
            loop {
                let (game, img) =
                    new_data(Arc::clone(&self.data), &mut self.previous_ids, self.osu_std).await;
                let _ = self
                    .channel
                    .send_message(&self.http, |m| {
                        let bytes: &[u8] = &img;
                        m.content("Here's the next one:")
                            .add_file((bytes, "bg_img.png"))
                    })
                    .await;
                while let Some(msg) = self.collector.receive_one().await {
                    debug!("Received: {}", msg.content);
                }
            }
        });
    }
}

// async fn process_result(
//     result: ContentResult,
//     channel: ChannelId,
//     http: &Arc<Http>,
//     game: &GameData,
// ) {
//     let content = match result {
//         ContentResult::Title { name, exact } => {
//             let mut content = if exact {
//                 format!("Gratz {}, you guessed it", name)
//             } else {
//                 format!("You were close enough {}, gratz", name)
//             };
//             let _ = write!(
//                 content,
//                 " \\:)\nMapset: https://osu.ppy.sh/beatmapsets/{}",
//                 game.mapset_id
//             );
//             content
//         }
//         ContentResult::Artist { name, exact } => {
//             if exact {
//                 format!(
//                     "That's the correct artist `{}`, can you get the title too?",
//                     name
//                 )
//             } else {
//                 format!(
//                     "`{}` got the artist almost correct, \
//                         it's actually `{}` but can you get the title?",
//                     name, game.artist
//                 )
//             }
//         }
//     };
//     let _ = channel.say(http, content).await;
// }

enum ContentResult {
    Title { exact: bool },
    Artist { exact: bool },
    None,
}

fn check_msg_content(content: &str, game: &GameData) -> ContentResult {
    // Guessed the title exactly?
    if content == &game.title {
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
    if !game.artist_guessed {
        // Guessed the artist exactly?
        if content == &game.artist {
            return ContentResult::Artist { exact: true };
        // Similar enough to the artist?
        } else if similarity < 0.3 && util::similarity(content, &game.artist) > 0.5 {
            return ContentResult::Artist { exact: false };
        }
    }
    ContentResult::None
}

async fn new_data(
    data: Arc<RwLock<ShareMap>>,
    previous_ids: &mut VecDeque<u32>,
    osu_std: bool,
) -> (GameData, Vec<u8>) {
    loop {
        match GameData::new(Arc::clone(&data), previous_ids, osu_std).await {
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

#[derive(Default)]
pub struct GameData {
    pub title: String,
    pub artist: String,
    pub artist_guessed: bool,
    pub mapset_id: u32,
    pub hints: Hints,
    pub reveal: ImageReveal,
}

impl GameData {
    pub async fn new(
        data: Arc<RwLock<ShareMap>>,
        previous_ids: &mut VecDeque<u32>,
        osu_std: bool,
    ) -> Result<Self, Error> {
        let mut path = PathBuf::from(env::var("BG_PATH")?);
        if !osu_std {
            path.push("mania");
        }
        let file_name = util::get_random_filename(previous_ids, &path)?;
        let mut split = file_name.split('.');
        let mapset_id = u32::from_str(split.next().unwrap()).unwrap();
        info!("Next BG mapset id: {}", mapset_id);
        let (title, artist) = util::get_title_artist(mapset_id, data).await?;
        let file_type = match split.next().unwrap() {
            "png" => ImageFormat::Png,
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            t => panic!("Can't read file type {}", t),
        };
        path.push(file_name);
        let img_vec = fs::read(path)?;
        let mut img = image::load_from_memory_with_format(&img_vec, file_type)?;
        let (w, h) = img.dimensions();
        // 800*600 (4:3)
        if w * h > 480_000 {
            img = img.resize(800, 600, FilterType::Lanczos3);
        }
        Ok(Self {
            hints: Hints::new(&title),
            title,
            artist,
            artist_guessed: false,
            mapset_id,
            reveal: ImageReveal::new(img),
        })
    }
}
