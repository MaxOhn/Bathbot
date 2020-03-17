use crate::{Error, MySQL, Osu};

use rand::RngCore;
use rosu::backend::BeatmapRequest;
use serenity::prelude::Context;
use std::{fs, path::PathBuf};
use tokio::runtime::Runtime;

pub fn get_random_filename(path: &PathBuf) -> Result<String, Error> {
    let mut files: Vec<String> = Vec::new();
    let dir_entries = fs::read_dir(path)?;
    for entry in dir_entries {
        if let Ok(entry) = entry {
            if let Ok(true) = entry.file_type().map(|ft| ft.is_file()) {
                files.push(entry.file_name().into_string().unwrap());
            }
        }
    }
    let mut rng = rand::thread_rng();
    let len = files.len();
    Ok(files
        .into_iter()
        .nth(rng.next_u32() as usize % len)
        .unwrap())
}

pub fn get_title_artist(mapset_id: u32, ctx: &Context) -> Result<(String, String), Error> {
    let (mut title, artist) = {
        let data = ctx.data.read();
        let mysql = data.get::<MySQL>().expect("Could not get MySQL");
        if let Ok(mapset) = mysql.get_beatmapset(mapset_id) {
            Ok((mapset.title, mapset.artist))
        } else {
            let request = BeatmapRequest::new().mapset_id(mapset_id);
            let mut rt = Runtime::new().unwrap();
            let osu = data.get::<Osu>().expect("Could not get Osu");
            match rt.block_on(request.queue_single(&osu)) {
                Ok(Some(map)) => Ok((map.title, map.artist)),
                _ => Err(Error::Custom(
                    "Could not retrieve map from osu API".to_string(),
                )),
            }
        }
    }?;
    if title.contains('(') && title.contains(')') {
        let idx_open = title.find('(').unwrap();
        let idx_close = title.find(')').unwrap();
        title.replace_range(idx_open..=idx_close, "");
    }
    if let Some(idx) = title.find("feat.").or_else(|| title.find("ft.")) {
        title.truncate(idx);
    }
    title = title.trim().to_string();
    Ok((title, artist))
}
