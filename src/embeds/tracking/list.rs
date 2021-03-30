use crate::util::constants::DESCRIPTION_SIZE;

use itertools::Itertools;
use rosu_v2::model::GameMode;
use std::fmt::Write;

pub struct TrackListEmbed {
    description: String,
    title: &'static str,
}

impl TrackListEmbed {
    pub fn new(users: Vec<(String, GameMode, usize)>) -> Vec<Self> {
        let mut embeds = Vec::new();
        let title = "Tracked osu! users in this channel (limit)";
        let mut description = String::with_capacity(256);

        users
            .into_iter()
            .group_by(|(_, mode, _)| *mode)
            .into_iter()
            .for_each(|(mode, group)| {
                let mode = match mode {
                    GameMode::STD => "osu!standard",
                    GameMode::MNA => "osu!mania",
                    GameMode::TKO => "osu!taiko",
                    GameMode::CTB => "osu!ctb",
                };

                description.reserve(256);
                let mut names = group.map(|(name, _, limit)| (name, limit));
                let (first_name, first_limit) = names.next().unwrap();
                let len = description.chars().count() + mode.len() + first_name.chars().count() + 7;

                if len > DESCRIPTION_SIZE {
                    embeds.push(Self {
                        title,
                        description: description.to_owned(),
                    });

                    description.clear();
                }

                let _ = writeln!(description, "__**{}**__", mode);
                let _ = write!(description, "`{}` ({})", first_name, first_limit);
                let mut with_comma = true;

                for (name, limit) in names {
                    let len = description.chars().count() + name.chars().count() + 9;

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
                        "{}`{}` ({})",
                        if with_comma { ", " } else { "" },
                        name,
                        limit,
                    );

                    with_comma = true;
                }

                description.push('\n');
            });

        if description.lines().count() > 1 {
            embeds.push(Self { description, title });
        }

        embeds
    }
}

impl_builder!(TrackListEmbed { description, title });
