use bathbot_model::{Effects, MapsetTags};
use bathbot_psql::model::games::DbMapTagsParams;
use bathbot_util::{constants::GENERAL_ISSUE, fields, EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::{Report, Result};
use futures::future::BoxFuture;
use rosu_v2::prelude::GameMode;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
        Component,
    },
    id::{
        marker::{ChannelMarker, UserMarker},
        Id,
    },
};

pub use self::game_wrapper::BackgroundGame;
use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::fun::GameDifficulty,
    core::Context,
    util::{interaction::InteractionComponent, Authored, ComponentExt},
};

mod game;
mod game_wrapper;
mod hints;
mod img_reveal;
mod mapset;
mod util;

pub struct BackgroundGameSetup {
    difficulty: GameDifficulty,
    effects: Effects,
    excluded: MapsetTags,
    included: MapsetTags,
    state: SetupState,
    msg_owner: Id<UserMarker>,
}

impl IActiveMessage for BackgroundGameSetup {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        if let SetupState::Ready { channel } = self.state {
            return Box::pin(self.start(channel));
        }

        let description = format!(
            "<@{}> select which tags should be included \
            and which ones should be excluded, then start the game. \
            Only you can use the components below.",
            self.msg_owner,
        );

        let mut fields = Vec::new();

        if !self.included.is_empty() {
            fields![fields { "Included tags", self.included.join(", "), false }];
        }

        if !self.excluded.is_empty() {
            fields![fields { "Excluded tags", self.excluded.join(", "), false }];
        }

        if !self.effects.is_empty() {
            fields![fields { "Effects", self.effects.join(", "), false }];
        }

        let embed = EmbedBuilder::new().description(description).fields(fields);

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        if let SetupState::Ready { .. } = self.state {
            return Vec::new();
        }

        let tag_options = |tags: MapsetTags| {
            vec![
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Easy),
                    description: None,
                    emoji: None,
                    label: "Easy".to_owned(),
                    value: "easy".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Hard),
                    description: None,
                    emoji: None,
                    label: "Hard".to_owned(),
                    value: "hard".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Meme),
                    description: None,
                    emoji: None,
                    label: "Meme".to_owned(),
                    value: "meme".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Weeb),
                    description: None,
                    emoji: None,
                    label: "Weeb".to_owned(),
                    value: "weeb".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Kpop),
                    description: None,
                    emoji: None,
                    label: "K-Pop".to_owned(),
                    value: "kpop".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Farm),
                    description: None,
                    emoji: None,
                    label: "Farm".to_owned(),
                    value: "farm".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::HardName),
                    description: None,
                    emoji: None,
                    label: "Hard name".to_owned(),
                    value: "hardname".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Alternate),
                    description: None,
                    emoji: None,
                    label: "Alternate".to_owned(),
                    value: "alt".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::BlueSky),
                    description: None,
                    emoji: None,
                    label: "Blue sky".to_owned(),
                    value: "bluesky".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::English),
                    description: None,
                    emoji: None,
                    label: "English".to_owned(),
                    value: "english".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Streams),
                    description: None,
                    emoji: None,
                    label: "Streams".to_owned(),
                    value: "streams".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Old),
                    description: None,
                    emoji: None,
                    label: "Old".to_owned(),
                    value: "old".to_owned(),
                },
                SelectMenuOption {
                    default: tags.contains(MapsetTags::Tech),
                    description: None,
                    emoji: None,
                    label: "Tech".to_owned(),
                    value: "tech".to_owned(),
                },
            ]
        };

        let include_options = tag_options(self.included);

        let include_menu = SelectMenu {
            custom_id: "bg_setup_include".to_owned(),
            disabled: false,
            max_values: Some(include_options.len() as u8),
            min_values: Some(0),
            options: include_options,
            placeholder: Some("Select which tags should be included".to_owned()),
        };

        let include_row = ActionRow {
            components: vec![Component::SelectMenu(include_menu)],
        };

        let exclude_options = tag_options(self.excluded);

        let exclude_menu = SelectMenu {
            custom_id: "bg_setup_exclude".to_owned(),
            disabled: false,
            max_values: Some(exclude_options.len() as u8),
            min_values: Some(0),
            options: exclude_options,
            placeholder: Some("Select which tags should be excluded".to_owned()),
        };

        let exclude_row = ActionRow {
            components: vec![Component::SelectMenu(exclude_menu)],
        };

        let effects = vec![
            SelectMenuOption {
                default: self.effects.contains(Effects::Blur),
                description: Some("Blur the image".to_owned()),
                emoji: None,
                label: "Blur".to_owned(),
                value: "blur".to_owned(),
            },
            SelectMenuOption {
                default: self.effects.contains(Effects::Contrast),
                description: Some("Increase the color contrast".to_owned()),
                emoji: None,
                label: "Contrast".to_owned(),
                value: "contrast".to_owned(),
            },
            SelectMenuOption {
                default: self.effects.contains(Effects::FlipHorizontal),
                description: Some("Flip the image horizontally".to_owned()),
                emoji: None,
                label: "Flip horizontal".to_owned(),
                value: "flip_h".to_owned(),
            },
            SelectMenuOption {
                default: self.effects.contains(Effects::FlipVertical),
                description: Some("Flip the image vertically".to_owned()),
                emoji: None,
                label: "Flip vertical".to_owned(),
                value: "flip_v".to_owned(),
            },
            SelectMenuOption {
                default: self.effects.contains(Effects::Grayscale),
                description: Some("Grayscale the colors".to_owned()),
                emoji: None,
                label: "Grayscale".to_owned(),
                value: "grayscale".to_owned(),
            },
            SelectMenuOption {
                default: self.effects.contains(Effects::Invert),
                description: Some("Invert the colors".to_owned()),
                emoji: None,
                label: "Invert".to_owned(),
                value: "invert".to_owned(),
            },
        ];

        let effects_menu = SelectMenu {
            custom_id: "bg_setup_effects".to_owned(),
            disabled: false,
            max_values: Some(effects.len() as u8),
            min_values: Some(0),
            options: effects,
            placeholder: Some("Modify images through effects".to_owned()),
        };

        let effects_row = ActionRow {
            components: vec![Component::SelectMenu(effects_menu)],
        };

        let start_button = Button {
            custom_id: Some("bg_start_button".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Start".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let cancel_button = Button {
            custom_id: Some("bg_cancel_button".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Cancel".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let button_row = ActionRow {
            components: vec![
                Component::Button(start_button),
                Component::Button(cancel_button),
            ],
        };

        vec![
            Component::ActionRow(include_row),
            Component::ActionRow(exclude_row),
            Component::ActionRow(effects_row),
            Component::ActionRow(button_row),
        ]
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err).boxed(),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore.boxed();
        }

        match component.data.custom_id.as_str() {
            "bg_setup_include" => self.included = MapsetTags::from(&*component),
            "bg_setup_exclude" => self.excluded = MapsetTags::from(&*component),
            "bg_setup_effects" => self.effects = Effects::from(&*component),
            "bg_start_button" => {
                self.state = SetupState::Ready {
                    channel: component.channel_id,
                }
            }
            "bg_cancel_button" => return Box::pin(self.cancel(component)),
            other => {
                warn!(name = %other, ?component, "Unknown background game setup component");

                return ComponentResult::Ignore.boxed();
            }
        }

        ComponentResult::BuildPage.boxed()
    }
}

impl BackgroundGameSetup {
    pub fn new(difficulty: GameDifficulty, msg_owner: Id<UserMarker>) -> Self {
        Self {
            difficulty,
            msg_owner,
            effects: Effects::empty(),
            excluded: MapsetTags::empty(),
            included: MapsetTags::empty(),
            state: SetupState::Ongoing,
        }
    }

    async fn start(&mut self, channel: Id<ChannelMarker>) -> Result<BuildPage> {
        if let Some(game) = Context::bg_games().write(&channel).await.remove() {
            if let Err(err) = game.stop() {
                warn!(?err, "Failed to stop previous game");
            }
        }

        let mut params = DbMapTagsParams::new(GameMode::Osu);

        params.include(self.included);
        params.exclude(self.excluded);

        let entries = match Context::games().bggame_tags(params).await {
            Ok(entries) => entries,
            Err(err) => {
                warn!(?err, "Failed to get background game tags");
                let embed = EmbedBuilder::new().color_red().description(GENERAL_ISSUE);

                return Ok(BuildPage::new(embed, true));
            }
        };

        let include_value = if !self.included.is_empty() {
            self.included.join('\n')
        } else if self.excluded.is_empty() {
            "Any".to_owned()
        } else {
            "None".to_owned()
        };

        let excluded_value = if !self.excluded.is_empty() {
            self.excluded.join('\n')
        } else {
            "None".to_owned()
        };

        let effects_value = if !self.effects.is_empty() {
            self.effects.join('\n')
        } else {
            "None".to_owned()
        };

        let fields = fields![
            "Included", include_value, true;
            "Excluded", excluded_value, true;
            "Effects", effects_value, true;
        ];

        let footer = FooterBuilder::new(format!("Difficulty: {:?}", self.difficulty));
        let title = format!("Selected tags ({} backgrounds)", entries.tags.len());

        let embed = EmbedBuilder::new()
            .fields(fields)
            .footer(footer)
            .title(title);

        if entries.tags.is_empty() {
            let description = "No stored backgrounds match these tags, try different ones";

            Ok(BuildPage::new(embed.description(description), false))
        } else {
            info!(
                included = self.included.join(','),
                excluded = self.excluded.join(','),
                "Starting game"
            );

            let game_fut = BackgroundGame::new(channel, entries, self.effects, self.difficulty);

            let game = game_fut.await;
            Context::bg_games().own(channel).await.insert(game);

            Ok(BuildPage::new(embed, false))
        }
    }

    async fn cancel(&mut self, component: &InteractionComponent) -> ComponentResult {
        let builder = MessageBuilder::new()
            .embed("Aborted background game setup")
            .components(Vec::new());

        match component.callback(builder).await {
            Ok(_) => ComponentResult::Ignore,
            Err(err) => {
                let wrap = "Failed to callback on background game setup cancel";

                ComponentResult::Err(Report::new(err).wrap_err(wrap))
            }
        }
    }
}

impl From<&InteractionComponent> for MapsetTags {
    fn from(component: &InteractionComponent) -> Self {
        component
            .data
            .values
            .iter()
            .fold(Self::empty(), |tags, value| {
                tags | match value.as_str() {
                    "easy" => Self::Easy,
                    "hard" => Self::Hard,
                    "meme" => Self::Meme,
                    "weeb" => Self::Weeb,
                    "kpop" => Self::Kpop,
                    "farm" => Self::Farm,
                    "hardname" => Self::HardName,
                    "alt" => Self::Alternate,
                    "bluesky" => Self::BlueSky,
                    "english" => Self::English,
                    "streams" => Self::Streams,
                    "old" => Self::Old,
                    "tech" => Self::Tech,
                    _ => {
                        warn!(%value, "Unknown mapset tag");

                        return tags;
                    }
                }
            })
    }
}

impl From<&InteractionComponent> for Effects {
    fn from(component: &InteractionComponent) -> Self {
        component
            .data
            .values
            .iter()
            .fold(Self::empty(), |effects, value| {
                effects
                    | match value.as_str() {
                        "blur" => Self::Blur,
                        "contrast" => Self::Contrast,
                        "flip_h" => Self::FlipHorizontal,
                        "flip_v" => Self::FlipVertical,
                        "grayscale" => Self::Grayscale,
                        "invert" => Self::Invert,
                        _ => {
                            warn!(%value, "Unknown effects");

                            return effects;
                        }
                    }
            })
    }
}

#[derive(Copy, Clone)]
enum SetupState {
    Ongoing,
    Ready { channel: Id<ChannelMarker> },
}
