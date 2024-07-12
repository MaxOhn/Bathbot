use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_util::constants::DESCRIPTION_SIZE;
use rosu_v2::model::GameMode;

use crate::commands::tracking::TracklistUserEntry;

#[derive(EmbedData)]
pub struct TrackListEmbed {
    description: String,
    title: &'static str,
}

impl TrackListEmbed {
    pub fn new(users: Vec<TracklistUserEntry>) -> Vec<Self> {
        const MODES: [GameMode; 4] = [
            GameMode::Osu,
            GameMode::Taiko,
            GameMode::Catch,
            GameMode::Mania,
        ];

        let mut embeds = Vec::new();
        let title = "Tracked osu! users in this channel (limit)";
        let mut description = String::with_capacity(256);

        // One group per mode
        let mut groups = [(); 4].map(|_| Vec::new());

        for entry in users {
            groups[entry.mode as usize].push(entry);
        }

        for (mode, group) in MODES.into_iter().zip(groups) {
            let mut names = group.into_iter().map(|entry| (entry.name, entry.limit));

            let Some((first_name, first_limit)) = names.next() else {
                continue;
            };

            let mode = match mode {
                GameMode::Osu => "osu!standard",
                GameMode::Mania => "osu!mania",
                GameMode::Taiko => "osu!taiko",
                GameMode::Catch => "osu!ctb",
            };

            description.reserve(256);
            let len = description.chars().count() + mode.len() + first_name.chars().count() + 7;

            if len > DESCRIPTION_SIZE {
                embeds.push(Self {
                    title,
                    description: description.to_owned(),
                });

                description.clear();
            }

            let _ = writeln!(description, "__**{mode}**__");
            let _ = write!(description, "`{first_name}` ({first_limit})");
            let mut with_comma = true;

            for (name, limit) in names {
                let len = description.chars().count() + name.chars().count() + 9;

                if len > DESCRIPTION_SIZE {
                    embeds.push(Self {
                        title,
                        description: description.to_owned(),
                    });

                    description.clear();
                    let _ = writeln!(description, "__**{mode}**__");
                    with_comma = false;
                }

                let _ = write!(
                    description,
                    "{}`{name}` ({limit})",
                    if with_comma { ", " } else { "" },
                );

                with_comma = true;
            }

            description.push('\n');
        }

        if description.lines().count() > 1 {
            embeds.push(Self { description, title });
        }

        embeds
    }
}
