use std::{
    fmt::{Debug, Formatter, Result as FmtResult, Write},
    future::ready,
};

use bathbot_model::rosu_v2::user::User;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{
    constants::OSU_BASE,
    datetime::{HowLongAgoDynamic, SecToMinSec},
    fields,
    numbers::round,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageOrigin,
};
use eyre::{Result, WrapErr};
use futures::future::BoxFuture;
use twilight_interactions::command::{CommandOption, CreateOption};
use twilight_model::{
    channel::message::{
        component::{ActionRow, SelectMenu, SelectMenuOption},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{response::ActiveResponse, BuildPage, ComponentResult, IActiveMessage},
    commands::{utility::ScoreEmbedBuilderData, ShowHideOption},
    embeds::{ComboFormatter, HitResultFormatter, PpFormatter},
    manager::redis::RedisData,
    util::{
        interaction::InteractionComponent,
        osu::{GradeCompletionFormatter, PersonalBestIndex, ScoreFormatter},
        Authored, Emote,
    },
};

pub struct ScoreEmbedBuilderActive {
    data: ScoreEmbedBuilderData,
    settings: ScoreEmbedBuilderSettings,
    msg_owner: Id<UserMarker>,

    author: AuthorBuilder,
    description: String,
    title: String,

    score_fmt: ScoreFormatter,
}

impl ScoreEmbedBuilderActive {
    pub fn new(
        user: RedisData<User>,
        data: ScoreEmbedBuilderData,
        settings: ScoreEmbedBuilderSettings,
        score_data: ScoreData,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        let author = user.author_builder();

        let personal_best = PersonalBestIndex::FoundScore { idx: 71 };
        let mut description = String::with_capacity(25);
        description.push_str("__**");

        if let Some(desc) =
            personal_best.into_embed_description(&MessageOrigin::new(None, Id::new(1)))
        {
            description.push_str(&desc);
            description.push_str(" and ");
        }

        description.push_str("Global Top #7**__");

        let title = format!(
            "{} - {} [{}] [{}★]",
            data.map.artist().cow_escape_markdown(),
            data.map.title().cow_escape_markdown(),
            data.map.version().cow_escape_markdown(),
            round(data.stars)
        );

        let score_fmt = ScoreFormatter::new(&data.score, score_data);

        Self {
            author,
            data,
            settings,
            msg_owner,
            description,
            title,
            score_fmt,
        }
    }
}

impl IActiveMessage for ScoreEmbedBuilderActive {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let ScoreEmbedBuilderData {
            score,
            map,
            if_fc,
            max_pp,
            stars: _,
            max_combo,
        } = &self.data;

        let combo = ComboFormatter::new(score.max_combo, Some(*max_combo));

        let mut name = format!(
            "{grade_completion_mods}\t{score_fmt}\t{acc}%\t",
            // We don't use `GradeCompletionFormatter::new` so that it doesn't
            // use the score id to hyperlink the grade because those don't
            // work in embed field names.
            grade_completion_mods = GradeCompletionFormatter::new_without_score(
                &score.mods,
                score.grade,
                score.total_hits(),
                map.mode(),
                map.n_objects()
            ),
            score_fmt = self.score_fmt,
            acc = round(self.data.score.accuracy),
        );

        let mut value = match (self.settings.footer, self.settings.timestamp) {
            (ShowHideOption::Show, ScoreEmbedBuilderTimestamp::ScoreDate) => {
                let _ = write!(name, "{combo}");

                let mut result = PpFormatter::new(Some(score.pp), Some(*max_pp)).to_string();

                if let Some(if_fc) = if_fc {
                    let _ = write!(result, " ~~({:.2}pp)~~", if_fc.pp);
                }

                result
            }
            (ShowHideOption::Hide, _) | (_, ScoreEmbedBuilderTimestamp::MapRankedDate) => {
                let _ = write!(name, "{}", HowLongAgoDynamic::new(&score.ended_at));

                let mut result = match self.settings.pp {
                    ScoreEmbedBuilderPp::Max => {
                        PpFormatter::new(Some(score.pp), Some(*max_pp)).to_string()
                    }
                    ScoreEmbedBuilderPp::IfFc => {
                        let mut result = String::with_capacity(17);
                        result.push_str("**");
                        let _ = write!(result, "{:.2}", score.pp);

                        if let Some(if_fc) = if_fc {
                            let _ = write!(result, "pp** ~~({:.2}pp)~~", if_fc.pp);
                        } else {
                            let _ = write!(result, "**/{:.2}PP", max_pp.max(score.pp));
                        }

                        result
                    }
                };

                let _ = write!(result, " • {combo}");

                result
            }
        };

        let _ = write!(
            value,
            " • {}",
            HitResultFormatter::new(score.mode, score.statistics.clone())
        );

        match self.settings.map_info {
            ShowHideOption::Show => {
                let map_attrs = map.attributes().mods(score.mods.clone()).build();
                let clock_rate = map_attrs.clock_rate as f32;
                let seconds_drain = (map.seconds_drain() as f32 / clock_rate) as u32;

                let _ = write!(
                    value,
                    "\n`{len}` • `CS: {cs} AR: {ar} OD: {od} HP: {hp}` • **{bpm}** BPM",
                    len = SecToMinSec::new(seconds_drain).pad_secs(),
                    cs = round(map_attrs.cs as f32),
                    ar = round(map_attrs.ar as f32),
                    od = round(map_attrs.od as f32),
                    hp = round(map_attrs.hp as f32),
                    bpm = map.bpm() * clock_rate,
                );
            }
            ShowHideOption::Hide => {}
        }

        let fields = fields![name, value, false];

        let url = format!("{OSU_BASE}b/{}", map.map_id());

        let mut builder = EmbedBuilder::new()
            .author(self.author.clone())
            .description(&self.description)
            .fields(fields)
            .title(&self.title)
            .url(url);

        match self.settings.image {
            ScoreEmbedBuilderImage::Image => builder = builder.image(map.cover()),
            ScoreEmbedBuilderImage::Thumbnail => builder = builder.thumbnail(map.thumbnail()),
            ScoreEmbedBuilderImage::None => {}
        }

        match self.settings.footer {
            ShowHideOption::Show => {
                let emote = Emote::from(score.mode).url();
                let footer = FooterBuilder::new(map.footer_text()).icon_url(emote);
                builder = builder.footer(footer).timestamp(score.ended_at);

                match self.settings.timestamp {
                    ScoreEmbedBuilderTimestamp::ScoreDate => {
                        builder = builder.timestamp(score.ended_at)
                    }
                    ScoreEmbedBuilderTimestamp::MapRankedDate => match map.ranked_date() {
                        Some(ranked_date) => builder = builder.timestamp(ranked_date),
                        None => {}
                    },
                }
            }
            ShowHideOption::Hide => {}
        }

        BuildPage::new(builder, false)
            .content("Embed preview:")
            .boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        let mut components = vec![
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_image".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: vec![
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Image".to_owned(),
                            value: "image".to_owned(),
                        },
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Thumbnail".to_owned(),
                            value: "thumbnail".to_owned(),
                        },
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "None".to_owned(),
                            value: "none".to_owned(),
                        },
                    ],
                    placeholder: Some(format!("Image (current: {:?})", self.settings.image)),
                })],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_map_info".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: vec![
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Show".to_owned(),
                            value: "show".to_owned(),
                        },
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Hide".to_owned(),
                            value: "hide".to_owned(),
                        },
                    ],
                    placeholder: Some(format!("Map Info (current: {:?})", self.settings.map_info)),
                })],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_footer".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: vec![
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Show".to_owned(),
                            value: "show".to_owned(),
                        },
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Hide".to_owned(),
                            value: "hide".to_owned(),
                        },
                    ],
                    placeholder: Some(format!("Footer (current: {:?})", self.settings.footer)),
                })],
            }),
        ];

        let create_pp_component = || {
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "score_embed_builder_pp".to_owned(),
                    disabled: false,
                    max_values: None,
                    min_values: None,
                    options: vec![
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "Max PP".to_owned(),
                            value: "max_pp".to_owned(),
                        },
                        SelectMenuOption {
                            default: false,
                            description: None,
                            emoji: None,
                            label: "If-FC PP".to_owned(),
                            value: "if_fc".to_owned(),
                        },
                    ],
                    placeholder: Some(format!("PP (current: {:?})", self.settings.pp)),
                })],
            })
        };

        match self.settings.footer {
            ShowHideOption::Show => {
                components.push(Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "score_embed_builder_timestamp".to_owned(),
                        disabled: false,
                        max_values: None,
                        min_values: None,
                        options: vec![
                            SelectMenuOption {
                                default: false,
                                description: None,
                                emoji: None,
                                label: "Score date".to_owned(),
                                value: "score_date".to_owned(),
                            },
                            SelectMenuOption {
                                default: false,
                                description: None,
                                emoji: None,
                                label: "Map ranked date".to_owned(),
                                value: "ranked_date".to_owned(),
                            },
                        ],
                        placeholder: Some(format!(
                            "Timestamp (current: {:?})",
                            self.settings.timestamp
                        )),
                    })],
                }));

                match self.settings.timestamp {
                    ScoreEmbedBuilderTimestamp::ScoreDate => {}
                    ScoreEmbedBuilderTimestamp::MapRankedDate => {
                        components.push(create_pp_component())
                    }
                }
            }
            ShowHideOption::Hide => components.push(create_pp_component()),
        }

        components
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

        let Some(value) = component.data.values.pop() else {
            return ComponentResult::Err(eyre!(
                "Missing value in score embed builder menu `{}`",
                component.data.custom_id
            ))
            .boxed();
        };

        match component.data.custom_id.as_str() {
            "score_embed_builder_image" => match ScoreEmbedBuilderImage::from_value(&value) {
                Some(image) => self.settings.image = image,
                None => {
                    return ComponentResult::Err(eyre!(
                        "Invalid value `{value}` for score embed builder menu `{}`",
                        component.data.custom_id
                    ))
                    .boxed()
                }
            },
            "score_embed_builder_pp" => match ScoreEmbedBuilderPp::from_value(&value) {
                Some(pp) => self.settings.pp = pp,
                None => {
                    return ComponentResult::Err(eyre!(
                        "Invalid value `{value}` for score embed builder menu `{}`",
                        component.data.custom_id
                    ))
                    .boxed()
                }
            },
            "score_embed_builder_timestamp" => match ScoreEmbedBuilderTimestamp::from_value(&value)
            {
                Some(timestamp) => self.settings.timestamp = timestamp,
                None => {
                    return ComponentResult::Err(eyre!(
                        "Invalid value `{value}` for score embed builder menu `{}`",
                        component.data.custom_id
                    ))
                    .boxed()
                }
            },
            "score_embed_builder_map_info" => {
                self.settings.map_info = match value.as_str() {
                    "show" => ShowHideOption::Show,
                    "hide" => ShowHideOption::Hide,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Invalid value `{value}` for score embed builder menu `{}`",
                            component.data.custom_id
                        ))
                        .boxed()
                    }
                }
            }
            "score_embed_builder_footer" => {
                self.settings.footer = match value.as_str() {
                    "show" => ShowHideOption::Show,
                    "hide" => ShowHideOption::Hide,
                    _ => {
                        return ComponentResult::Err(eyre!(
                            "Invalid value `{value}` for score embed builder menu `{}`",
                            component.data.custom_id
                        ))
                        .boxed()
                    }
                }
            }
            other => {
                warn!(name = %other, ?component, "Unknown score embed builder component");

                return ComponentResult::Ignore.boxed();
            }
        }

        ComponentResult::BuildPage.boxed()
    }

    fn on_timeout(&mut self, response: ActiveResponse) -> BoxFuture<'_, Result<()>> {
        let builder = bathbot_util::MessageBuilder::new().components(Vec::new());
        match response.update(builder) {
            Some(update_fut) => {
                let fut = async {
                    update_fut
                        .await
                        .map(|_| ())
                        .wrap_err("Failed to remove components")
                };
                Box::pin(fut)
            }
            None => Box::pin(ready(Err(eyre!(
                "Lacking permission to update message on timeout"
            )))),
        }
    }
}

pub struct ScoreEmbedBuilderSettings {
    pub image: ScoreEmbedBuilderImage,
    pub pp: ScoreEmbedBuilderPp,
    pub map_info: ShowHideOption,
    pub footer: ShowHideOption,
    pub timestamp: ScoreEmbedBuilderTimestamp,
}

#[derive(Copy, Clone, CommandOption, CreateOption, Debug, Eq, PartialEq)]
pub enum ScoreEmbedBuilderImage {
    #[option(name = "Image", value = "image")]
    Image,
    #[option(name = "Thumbnail", value = "thumbnail")]
    Thumbnail,
    #[option(name = "None", value = "none")]
    None,
}

impl ScoreEmbedBuilderImage {
    fn from_value(value: &str) -> Option<Self> {
        match value {
            "image" => Some(Self::Image),
            "thumbnail" => Some(Self::Thumbnail),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum ScoreEmbedBuilderPp {
    #[option(name = "Max PP", value = "max_pp")]
    Max,
    #[option(name = "If-FC PP", value = "if_fc")]
    IfFc,
}

impl ScoreEmbedBuilderPp {
    fn from_value(value: &str) -> Option<Self> {
        match value {
            "max_pp" => Some(Self::Max),
            "if_fc" => Some(Self::IfFc),
            _ => None,
        }
    }
}

impl Debug for ScoreEmbedBuilderPp {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Max => f.write_str("Max PP"),
            Self::IfFc => f.write_str("If-FC PP"),
        }
    }
}

#[derive(Copy, Clone, CommandOption, CreateOption, Eq, PartialEq)]
pub enum ScoreEmbedBuilderTimestamp {
    #[option(name = "Score date", value = "score_date")]
    ScoreDate,
    #[option(name = "Map ranked date", value = "ranked_date")]
    MapRankedDate,
}

impl ScoreEmbedBuilderTimestamp {
    fn from_value(value: &str) -> Option<Self> {
        match value {
            "score_date" => Some(Self::ScoreDate),
            "ranked_date" => Some(Self::MapRankedDate),
            _ => None,
        }
    }
}

impl Debug for ScoreEmbedBuilderTimestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::ScoreDate => write!(f, "Score date"),
            Self::MapRankedDate => write!(f, "Map ranked date"),
        }
    }
}
