use std::{collections::HashSet, fmt::Write};

use bathbot_psql::model::configs::GuildConfig;
use bathbot_util::{
    constants::{BATHBOT_ROADMAP, BATHBOT_WORKSHOP},
    EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::{
        component::{ActionRow, SelectMenu, SelectMenuOption, SelectMenuType},
        Component, EmojiReactionType,
    },
    id::{marker::GuildMarker, Id},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    core::{
        commands::{
            interaction::InteractionCommands,
            prefix::{PrefixCommandGroup, PrefixCommands},
        },
        Context,
    },
    util::{interaction::InteractionComponent, Emote},
};

pub struct HelpPrefixMenu {
    current_group: Option<PrefixCommandGroup>,
    guild: Option<Id<GuildMarker>>,
}

impl IActiveMessage for HelpPrefixMenu {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let Some(group) = self.current_group else {
            return Box::pin(self.handle_general());
        };

        let mut cmds: Vec<_> = {
            let mut dedups = HashSet::new();

            PrefixCommands::get()
                .iter()
                .filter(|cmd| cmd.group == group)
                .filter(|cmd| dedups.insert(cmd.name()))
                .collect()
        };

        cmds.sort_unstable_by_key(|cmd| cmd.name());

        let mut desc = String::with_capacity(64);

        let emote = group.emote();
        let name = group.name();
        let _ = writeln!(desc, "{emote} __**{name}**__");

        for cmd in cmds {
            let name = cmd.name();
            let authority = if cmd.flags.authority() { "**\\***" } else { "" };
            let _ = writeln!(desc, "`{name}`{authority}: {}", cmd.desc);
        }

        let footer = FooterBuilder::new(
            "*: Either can't be used in DMs or requires authority status in the server",
        );

        let embed = EmbedBuilder::new().description(desc).footer(footer);

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        let options = vec![
            SelectMenuOption {
                default: self.current_group.is_none(),
                description: None,
                emoji: Some(EmojiReactionType::Unicode {
                    name: "🛁".to_owned(),
                }),
                label: "General".to_owned(),
                value: "general".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Osu)),
                description: None,
                emoji: Some(Emote::Std.reaction_type()),
                label: "osu!".to_owned(),
                value: "osu".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Taiko)),
                description: None,
                emoji: Some(Emote::Tko.reaction_type()),
                label: "Taiko".to_owned(),
                value: "taiko".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Catch)),
                description: None,
                emoji: Some(Emote::Ctb.reaction_type()),
                label: "Catch".to_owned(),
                value: "ctb".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Mania)),
                description: None,
                emoji: Some(Emote::Mna.reaction_type()),
                label: "Mania".to_owned(),
                value: "mania".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::AllModes)),
                description: None,
                emoji: Some(Emote::Osu.reaction_type()),
                label: "All Modes".to_owned(),
                value: "all_modes".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Tracking)),
                description: None,
                emoji: Some(Emote::Tracking.reaction_type()),
                label: "Tracking".to_owned(),
                value: "tracking".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Twitch)),
                description: None,
                emoji: Some(Emote::Twitch.reaction_type()),
                label: "Twitch".to_owned(),
                value: "twitch".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Games)),
                description: None,
                emoji: Some(EmojiReactionType::Unicode {
                    name: "🎮".to_owned(),
                }),
                label: "Games".to_owned(),
                value: "games".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Utility)),
                description: None,
                emoji: Some(EmojiReactionType::Unicode {
                    name: "🛠️".to_owned(),
                }),
                label: "Utility".to_owned(),
                value: "utility".to_owned(),
            },
            SelectMenuOption {
                default: matches!(self.current_group, Some(PrefixCommandGroup::Songs)),
                description: None,
                emoji: Some(EmojiReactionType::Unicode {
                    name: "🎵".to_owned(),
                }),
                label: "Songs".to_owned(),
                value: "songs".to_owned(),
            },
        ];

        let category = SelectMenu {
            custom_id: "help_category".to_owned(),
            disabled: false,
            max_values: Some(1),
            min_values: Some(1),
            options: Some(options),
            placeholder: None,
            channel_types: None,
            default_values: None,
            kind: SelectMenuType::Text,
        };

        let category_row = ActionRow {
            components: vec![Component::SelectMenu(category)],
        };

        vec![Component::ActionRow(category_row)]
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        let Some(value) = component.data.values.pop() else {
            return ComponentResult::Err(eyre!("Missing help menu value")).boxed();
        };

        self.current_group = match value.as_str() {
            "general" => None,
            "osu" => Some(PrefixCommandGroup::Osu),
            "taiko" => Some(PrefixCommandGroup::Taiko),
            "ctb" => Some(PrefixCommandGroup::Catch),
            "mania" => Some(PrefixCommandGroup::Mania),
            "all_modes" => Some(PrefixCommandGroup::AllModes),
            "tracking" => Some(PrefixCommandGroup::Tracking),
            "twitch" => Some(PrefixCommandGroup::Twitch),
            "games" => Some(PrefixCommandGroup::Games),
            "utility" => Some(PrefixCommandGroup::Utility),
            "songs" => Some(PrefixCommandGroup::Songs),
            other => {
                warn!(name = %other, ?component, "Unknown help menu component");

                return ComponentResult::Ignore.boxed();
            }
        };

        ComponentResult::BuildPage.boxed()
    }
}

impl HelpPrefixMenu {
    pub fn new(guild: Option<Id<GuildMarker>>) -> Self {
        Self {
            current_group: None,
            guild,
        }
    }

    async fn handle_general(&self) -> Result<BuildPage> {
        let (custom_prefix, first_prefix) = if let Some(guild_id) = self.guild {
            let f = |config: &GuildConfig| {
                let prefixes = &config.prefixes;

                if let Some(prefix) = prefixes.first().cloned() {
                    if prefix == GuildConfig::DEFAULT_PREFIX && prefixes.len() == 1 {
                        (None, prefix)
                    } else {
                        let prefix_iter = prefixes.iter().skip(1);
                        let mut prefixes_str = String::with_capacity(9);
                        let _ = write!(prefixes_str, "`{prefix}`");

                        for prefix in prefix_iter {
                            let _ = write!(prefixes_str, ", `{prefix}`");
                        }

                        (Some(prefixes_str), prefix)
                    }
                } else {
                    (None, GuildConfig::DEFAULT_PREFIX.into())
                }
            };

            Context::guild_config().peek(guild_id, f).await
        } else {
            (None, GuildConfig::DEFAULT_PREFIX.into())
        };

        let prefix_desc = custom_prefix.map_or_else(
            || {
                format!(
                    "Prefix: `{}` (none required in DMs)",
                    GuildConfig::DEFAULT_PREFIX
                )
            },
            |p| {
                format!(
                    "Server prefix: {p}\nDM prefix: `{}` or none at all",
                    GuildConfig::DEFAULT_PREFIX
                )
            },
        );

        let link = InteractionCommands::get_command("link").map_or_else(
            || "`/link`".to_owned(),
            |cmd| cmd.mention("link").to_string(),
        );

        let description = format!(
            ":fire: **Slash commands are supported!** Type `/` to check them out :fire:\n\n\
            {prefix_desc}\n\
            __**General**__\n\
            - To find out more about a command like what arguments you can give or which shorter aliases \
            it has,  use __**`{first_prefix}help [command]`**__, e.g. `{first_prefix}help simulate`. \n\
            - If you want to specify an argument, e.g. a username, that contains \
            spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
            - If you've used the {link} command to connect to an osu! account, \
            you can omit the username for any command that needs one.\n\
            - If you have questions, complains, or suggestions for the bot, feel free to join its \
            [discord server]({BATHBOT_WORKSHOP}) and let Badewanne3 know.\n\
            [This roadmap]({BATHBOT_ROADMAP}) shows already suggested features and known bugs.\n\n\
            __**Mods for osu!**__
            Many commands allow you to specify mods. You can do so with `+mods` \
            for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. \n\
            For example:\n\
            `+hdhr`: scores that include at least HD and HR\n\
            `+hd!`: only HD scores\n\
            `-nm!`: scores that are not NoMod\n\
            `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ"
        );

        let embed = EmbedBuilder::new().description(description);

        Ok(BuildPage::new(embed, false))
    }
}
