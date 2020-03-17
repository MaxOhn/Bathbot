use super::{util, Hints, ImageReveal};
use crate::{DispatchEvent, Error};

use hey_listen::sync::{
    ParallelDispatcherRequest as DispatcherRequest, ParallelListener as Listener,
};
use image::ImageFormat;
use serenity::{http::client::Http, model::id::ChannelId, prelude::Context};
use std::{env, fs, path::PathBuf, str::FromStr, sync::Arc};

pub struct BackGroundGame {
    title: String,
    artist: String,
    pub mapset_id: u32,
    hints: Hints,
    reveal: ImageReveal,
    channel: ChannelId,
    http: Arc<Http>,
}

impl BackGroundGame {
    pub fn new(ctx: &Context, channel: ChannelId, http: Arc<Http>) -> Result<Self, Error> {
        let (title, artist, mapset_id, hints, reveal) = Self::create(ctx)?;
        Ok(Self {
            hints,
            reveal,
            title,
            artist,
            mapset_id,
            channel,
            http,
        })
    }

    fn create(ctx: &Context) -> Result<(String, String, u32, Hints, ImageReveal), Error> {
        let mut path = PathBuf::from(env::var("BG_PATH")?);
        let file_name = util::get_random_filename(&path)?;
        let mut split = file_name.split('.');
        let mapset_id = u32::from_str(split.next().unwrap()).unwrap();
        let (title, artist) = util::get_title_artist(mapset_id, &ctx)?;
        let file_type = match split.next().unwrap() {
            "png" => ImageFormat::Png,
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            t => panic!("Can't read file type {}", t),
        };
        path.push(file_name);
        let img_vec = fs::read(path)?;
        let img = image::load_from_memory_with_format(&img_vec, file_type)?;
        let hints = Hints::new(&title);
        let reveal = ImageReveal::new(img);
        Ok((title, artist, mapset_id, hints, reveal))
    }

    pub fn increase_radius(&mut self) {
        self.reveal.increase_radius();
    }

    pub fn sub_image(&self) -> Result<Vec<u8>, Error> {
        self.reveal.sub_image()
    }

    pub fn reveal(&self) -> Result<Vec<u8>, Error> {
        self.reveal.full()
    }

    pub fn hint(&mut self) -> String {
        self.hints.get(&self.title, &self.artist)
    }
}

impl Listener<DispatchEvent> for BackGroundGame {
    fn on_event(&mut self, event: &DispatchEvent) -> Option<DispatcherRequest> {
        match event {
            DispatchEvent::BgMsgEvent {
                channel,
                user,
                content,
            } => {
                println!("Content: {}", content);
                None
            }
        }
    }
}
