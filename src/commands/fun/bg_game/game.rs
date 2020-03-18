use super::{util, Hints, ImageReveal};
use crate::{DispatchEvent, Error};

use hey_listen::sync::{
    ParallelDispatcherRequest as DispatcherRequest, ParallelListener as Listener,
};
use image::{imageops::FilterType, GenericImageView, ImageFormat};
use serenity::{
    cache::CacheRwLock,
    framework::standard::CommandResult,
    http::client::Http,
    model::id::{ChannelId, UserId},
    prelude::{Context, RwLock as SRwLock, ShareMap},
};
use std::{collections::VecDeque, env, fs, path::PathBuf, str::FromStr, sync::Arc};

pub struct BackGroundGame {
    game: GameData,
    previous_ids: VecDeque<u32>,
    channel: ChannelId,
    http: Arc<Http>,
    data: Arc<SRwLock<ShareMap>>,
    cache: CacheRwLock,
}

impl BackGroundGame {
    pub fn new(ctx: &Context, channel: ChannelId) -> Self {
        Self {
            game: GameData::default(),
            previous_ids: VecDeque::with_capacity(10),
            channel,
            http: Arc::clone(&ctx.http),
            data: Arc::clone(&ctx.data),
            cache: ctx.cache.clone(),
        }
    }

    pub fn restart(&mut self) -> CommandResult {
        self.resolve(None)?;
        self.game = GameData::new(Arc::clone(&self.data), &mut self.previous_ids)?;
        let img = self.game.reveal.sub_image()?;
        self.channel.send_message(&self.http, |m| {
            let bytes: &[u8] = &img;
            m.content("Here's the next one:")
                .add_file((bytes, "bg_img.png"))
        })?;
        Ok(())
    }

    pub fn increase_sub_image(&mut self) -> Result<Vec<u8>, Error> {
        self.game.reveal.increase_radius();
        self.game.reveal.sub_image()
    }

    pub fn reveal(&self) -> Result<Vec<u8>, Error> {
        self.game.reveal.full()
    }

    pub fn hint(&mut self) -> String {
        self.game.hints.get(&self.game.title, &self.game.artist)
    }

    pub fn resolve(&mut self, winner_msg: Option<String>) -> CommandResult {
        if self.game.mapset_id == 0 {
            return Ok(());
        };
        let content = winner_msg.unwrap_or_else(|| {
            format!(
                "Full background: https://osu.ppy.sh/beatmapsets/{}",
                self.game.mapset_id
            )
        });
        let bytes: &[u8] = &self.reveal()?;
        self.game.mapset_id = 0;
        let _ = self.channel.send_message(&self.http, |m| {
            m.add_file((bytes, "bg_img.png")).content(content)
        })?;
        Ok(())
    }

    fn user_name(&self, user_id: UserId) -> Option<String> {
        user_id
            .to_user((&self.cache, &*self.http))
            .ok()
            .map(|user| user.name)
    }

    fn process_winner(&mut self, winner: UserId, exact: bool) -> Option<DispatcherRequest> {
        let winner_name = self
            .user_name(winner)
            .map_or_else(String::new, |name| format!(" `{}`", name));
        let mut winner_msg = if exact {
            format!("Gratz{}, you guessed it", winner_name)
        } else {
            format!("You were close enough{}, gratz", winner_name)
        };
        winner_msg = format!(
            "{} \\:)\n\
            Mapset: https://osu.ppy.sh/beatmapsets/{}",
            winner_msg, self.game.mapset_id
        );
        if let Err(why) = self.resolve(Some(winner_msg)) {
            error!("Error while resolving game: {:?}", why);
        }
        if let Err(why) = self.restart() {
            error!("Error while restarting game: {:?}", why);
        }
        None
    }
}

impl Listener<DispatchEvent> for BackGroundGame {
    fn on_event(&mut self, event: &DispatchEvent) -> Option<DispatcherRequest> {
        match event {
            DispatchEvent::BgMsgEvent { user, content, .. } => {
                println!("> Content: {}", content);
                // Guessed the title exactly?
                if content == &self.game.title {
                    return self.process_winner(*user, true);
                }
                // Guessed sufficiently many words of the title?
                if self.game.title.contains(&" ") {
                    let mut same_word_len = 0;
                    for title_word in self.game.title.split(' ') {
                        for content_word in content.split(' ') {
                            if title_word == content_word {
                                same_word_len += title_word.len();
                                if same_word_len > 8 {
                                    return self.process_winner(*user, false);
                                }
                            }
                        }
                    }
                }
                // Similar enough to the title?
                let similarity = util::similarity(content, &self.game.title);
                if similarity > 0.5 {
                    return self.process_winner(*user, false);
                }
                if !self.game.artist_guessed {
                    // Guessed the artist exactly?
                    if content == &self.game.artist {
                        if let Some(user_name) = self.user_name(*user) {
                            let _ = self.channel.say(
                                &self.http,
                                format!(
                                    "That's the correct artist `{}`, can you get the title too?",
                                    user_name
                                ),
                            );
                            self.game.artist_guessed = true;
                            return None;
                        }
                    // Similar enough to the artist?
                    } else if similarity < 0.3 && util::similarity(content, &self.game.artist) > 0.5
                    {
                        if let Some(user_name) = self.user_name(*user) {
                            let _ = self.channel.say(
                                &self.http,
                                format!(
                                    "`{}` got the artist almost correct, \
                                    it's actually `{}` but can you get the title?",
                                    user_name, self.game.artist
                                ),
                            );
                            self.game.artist_guessed = true;
                            return None;
                        }
                    }
                }
                None
            }
        }
    }
}

#[derive(Default)]
struct GameData {
    pub title: String,
    pub artist: String,
    pub artist_guessed: bool,
    pub mapset_id: u32,
    pub hints: Hints,
    pub reveal: ImageReveal,
}

impl GameData {
    fn new(data: Arc<SRwLock<ShareMap>>, previous_ids: &mut VecDeque<u32>) -> Result<Self, Error> {
        let mut path = PathBuf::from(env::var("BG_PATH")?);
        let file_name = util::get_random_filename(previous_ids, &path)?;
        let mut split = file_name.split('.');
        let mapset_id = u32::from_str(split.next().unwrap()).unwrap();
        let (title, artist) = util::get_title_artist(mapset_id, data)?;
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
