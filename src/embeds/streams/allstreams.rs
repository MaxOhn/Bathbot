use crate::embeds::EmbedData;

use itertools::Itertools;
use serenity::model::{gateway::Presence, id::UserId};
use std::{collections::HashMap, fmt::Write};

#[derive(Clone)]
pub struct AllStreamsEmbed {
    description: String,
    thumbnail: Option<String>,
    title: String,
}

impl AllStreamsEmbed {
    pub fn new(
        presences: Vec<Presence>,
        users: HashMap<UserId, String>,
        total: usize,
        thumbnail: Option<String>,
    ) -> Self {
        let mut title = format!("{} current streamers on this server:", total);
        // No streamers -> Simple message
        let description = if presences.is_empty() {
            "No one is currently streaming".to_string()
        // Less than 20 streamers -> Descriptive single column
        } else if presences.len() <= 20 {
            let mut description = String::with_capacity(512);
            for p in presences {
                let activity = p.activity.expect("activity");
                let _ = write!(
                    description,
                    "- {name} playing {game}",
                    name = users.get(&p.user_id).unwrap(),
                    game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"))
                );
                if let Some(title) = activity.details {
                    if let Some(url) = activity.url {
                        let _ = writeln!(description, ": [{}]({})", title, url);
                    } else {
                        let _ = writeln!(description, ": {}", title);
                    }
                } else {
                    description.push('\n');
                }
            }
            description
        // Less than 40 streamers -> Two simple columns
        } else if presences.len() <= 40 {
            let mut description = String::with_capacity(768);
            for mut chunk in presences.into_iter().chunks(2).into_iter() {
                // First
                let first: Presence = chunk.next().unwrap();
                let activity = first.activity.unwrap();
                let _ = write!(
                    description,
                    "- {name}: ",
                    name = users.get(&first.user_id).unwrap(),
                );
                let game = activity
                    .state
                    .unwrap_or_else(|| panic!("Could not get state of activity"));
                if let Some(url) = activity.url {
                    let _ = write!(description, "[{}]({})", game, url);
                } else {
                    description.push_str(&game);
                }
                // Second
                if let Some(second) = chunk.next() {
                    let _ = write!(
                        description,
                        " ~ {name}: ",
                        name = users.get(&second.user_id).unwrap()
                    );
                    let activity = second.activity.unwrap();
                    let game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"));
                    if let Some(url) = activity.url {
                        let _ = write!(description, "[{}]({})", game, url);
                    } else {
                        description.push_str(&game);
                    }
                    description.push('\n');
                }
            }
            description
        // 40+ Streamers -> Three simple columns
        } else {
            if presences.len() == 60 {
                title = format!("60 out of {} current streamers of this server:", total);
            }
            let mut description = String::with_capacity(1024);
            for mut chunk in presences.into_iter().chunks(3).into_iter() {
                // First
                let first: Presence = chunk.next().unwrap();
                let activity = first.activity.unwrap();
                let _ = write!(
                    description,
                    "- {name}: ",
                    name = users.get(&first.user_id).unwrap(),
                );
                let game = activity
                    .state
                    .unwrap_or_else(|| panic!("Could not get state of activity"));
                if let Some(url) = activity.url {
                    let _ = write!(description, "[{}]({})", game, url);
                } else {
                    description.push_str(&game);
                }
                // Second
                if let Some(second) = chunk.next() {
                    let _ = write!(
                        description,
                        " ~ {name}: ",
                        name = users.get(&second.user_id).unwrap()
                    );
                    let activity = second.activity.unwrap();
                    let game = activity
                        .state
                        .unwrap_or_else(|| panic!("Could not get state of activity"));
                    if let Some(url) = activity.url {
                        let _ = write!(description, "[{}]({})", game, url);
                    } else {
                        description.push_str(&game);
                    }
                    // Third
                    if let Some(third) = chunk.next() {
                        let _ = write!(
                            description,
                            " ~ {name}: ",
                            name = users.get(&third.user_id).unwrap()
                        );
                        let activity = third.activity.unwrap();
                        let game = activity
                            .state
                            .unwrap_or_else(|| panic!("Could not get state of activity"));
                        if let Some(url) = activity.url {
                            let _ = write!(description, "[{}]({})", game, url);
                        } else {
                            description.push_str(&game);
                        }
                        description.push('\n');
                    }
                }
            }
            description
        };
        Self {
            description,
            thumbnail,
            title,
        }
    }
}

impl EmbedData for AllStreamsEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn thumbnail(&self) -> Option<&str> {
        self.thumbnail.as_deref()
    }
}
