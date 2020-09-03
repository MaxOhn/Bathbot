use crate::{embeds::EmbedData, util::constants::DESCRIPTION_SIZE};

use itertools::Itertools;
use rosu::models::GameMode;
use std::fmt::Write;

#[derive(Clone)]
pub struct TrackListEmbed {
    title: &'static str,
    description: String,
}

impl TrackListEmbed {
    pub fn new(users: Vec<(String, GameMode)>) -> Vec<Self> {
        let mut embeds = Vec::with_capacity(1);
        let title = "Tracked osu! users in this channel";
        let mut description = String::with_capacity(256);
        users
            .into_iter()
            .group_by(|(_, mode)| *mode)
            .into_iter()
            .for_each(|(mode, group)| {
                let mode = match mode {
                    GameMode::STD => "osu!standard",
                    GameMode::MNA => "osu!mania",
                    GameMode::TKO => "osu!taiko",
                    GameMode::CTB => "osu!ctb",
                };
                description.reserve(256);
                let mut names = group.map(|(name, _)| name);
                let first = names.next().unwrap();
                let len = description.chars().count() + mode.len() + first.chars().count() + 2;
                if len > DESCRIPTION_SIZE {
                    embeds.push(Self {
                        title,
                        description: description.to_owned(),
                    });
                    description.clear();
                }
                let _ = writeln!(description, "__**{}**__", mode);
                let _ = write!(description, "`{}`", first);
                let mut with_comma = true;
                for name in names {
                    let len = description.chars().count() + name.chars().count() + 4;
                    if len > DESCRIPTION_SIZE {
                        embeds.push(Self {
                            title,
                            description: description.to_owned(),
                        });
                        description.clear();
                        let _ = writeln!(description, "__**{}**__", mode);
                        with_comma = false;
                    }
                    let _ = write!(
                        description,
                        "{}`{}`",
                        if with_comma { ", " } else { "" },
                        name
                    );
                    with_comma = true;
                }
            });
        embeds
    }
}

impl EmbedData for TrackListEmbed {
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }
    fn description(&self) -> Option<&str> {
        Some(&self.title)
    }
}
