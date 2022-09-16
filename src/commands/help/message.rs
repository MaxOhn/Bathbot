use std::{collections::BTreeMap, fmt::Write, sync::Arc};

use command_macros::command;
use eyre::{ContextCompat, Report, Result};
use hashbrown::HashSet;
use twilight_model::{
    application::component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
    channel::{embed::EmbedField, Message, ReactionType},
    id::{marker::GuildMarker, Id},
};

use crate::{
    core::{
        commands::prefix::{PrefixCommand, PrefixCommandGroup, PrefixCommands},
        Context,
    },
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder},
        constants::{BATHBOT_ROADMAP, BATHBOT_WORKSHOP},
        interaction::InteractionComponent,
        levenshtein_distance, ChannelExt, ComponentExt, Emote,
    },
};

use super::failed_message_content;

#[command]
#[desc("Display help for prefix commands")]
#[group(Utility)]
#[alias("h")]
#[usage("[command]")]
#[example("", "recent", "osg")]
async fn prefix_help(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> Result<()> {
    match args.next() {
        Some(arg) => match PrefixCommands::get().command(arg) {
            Some(cmd) => command_help(ctx, msg, cmd).await,
            None => failed_help(ctx, msg, arg).await,
        },
        None => dm_help(ctx, msg).await,
    }
}

async fn failed_help(ctx: Arc<Context>, msg: &Message, name: &str) -> Result<()> {
    let mut seen = HashSet::new();

    let dists: BTreeMap<_, _> = PrefixCommands::get()
        .iter()
        .filter(|cmd| seen.insert(cmd.name()))
        .flat_map(|cmd| cmd.names.iter())
        .map(|&cmd| (levenshtein_distance(name, cmd).0, cmd))
        .filter(|(dist, _)| *dist < 4)
        .collect();

    let content = failed_message_content(dists);
    msg.error(&ctx, content).await?;

    Ok(())
}

async fn command_help(ctx: Arc<Context>, msg: &Message, cmd: &PrefixCommand) -> Result<()> {
    let name = cmd.name();
    let prefix = ctx.guild_first_prefix(msg.guild_id).await;
    let mut fields = Vec::new();

    let mut eb = EmbedBuilder::new()
        .title(name)
        .description(cmd.help.unwrap_or(cmd.desc));

    let mut usage_len = 0;

    if let Some(usage) = cmd.usage {
        let value = format!("`{prefix}{name} {usage}`");
        usage_len = value.chars().count();

        let field = EmbedField {
            name: "How to use".to_owned(),
            value,
            inline: usage_len <= 29,
        };

        fields.push(field);
    }

    let mut examples = cmd.examples.iter();

    if let Some(first) = examples.next() {
        let len: usize = cmd.examples.iter().map(|&e| name.len() + e.len() + 4).sum();
        let mut value = String::with_capacity(len);
        let mut example_len = 0;
        let cmd_len = prefix.chars().count() + name.chars().count();
        writeln!(value, "`{prefix}{name} {first}`")?;

        for example in examples {
            writeln!(value, "`{prefix}{name} {example}`")?;
            example_len = example_len.max(cmd_len + example.chars().count());
        }

        let not_inline = (usage_len <= 29 && cmd.names.len() > 1 && example_len > 27)
            || ((usage_len > 29 || cmd.names.len() > 1) && example_len > 36)
            || example_len > 45;

        let field = EmbedField {
            name: "Examples".to_owned(),
            value,
            inline: !not_inline,
        };

        fields.push(field);
    }

    let mut aliases = cmd.names.iter().skip(1);

    if let Some(first) = aliases.next() {
        let len: usize = cmd.names.iter().skip(1).map(|n| 4 + n.len()).sum();
        let mut value = String::with_capacity(len);
        write!(value, "`{first}`")?;

        for &alias in aliases {
            write!(value, ", `{alias}`")?;
        }

        let field = EmbedField {
            name: "Aliases".to_owned(),
            value,
            inline: true,
        };

        fields.push(field);
    }

    if cmd.flags.authority() {
        let value = if let Some(guild_id) = msg.guild_id {
            let authorities = ctx.guild_authorities(guild_id).await;
            let mut value = "You need admin permission".to_owned();
            let mut iter = authorities.iter();

            if let Some(first) = iter.next() {
                let _ = write!(value, " or any of these roles: <@&{first}>");

                for role in iter {
                    let _ = write!(value, ", <@&{role}>");
                }
            }

            value
        } else {
            "Admin permission or any role that \
            was setup as authority in a server"
                .to_owned()
        };

        let field = EmbedField {
            name: "Requires authority status".to_owned(),
            value,
            inline: false,
        };

        fields.push(field);
    }

    if cmd.flags.only_owner() {
        let author = AuthorBuilder::new("Can only be used by the bot owner");
        eb = eb.author(author);
    }

    let footer_text = if cmd.flags.only_guilds() || cmd.flags.authority() {
        "Only available in servers"
    } else {
        "Available in servers and DMs"
    };

    let footer = FooterBuilder::new(footer_text);

    let embed = eb.footer(footer).fields(fields).build();
    let builder = MessageBuilder::new().embed(embed);

    msg.create_message(&ctx, &builder).await?;

    Ok(())
}

async fn description(ctx: &Context, guild_id: Option<Id<GuildMarker>>) -> String {
    let (custom_prefix, first_prefix) = if let Some(guild_id) = guild_id {
        let mut prefixes = ctx.guild_prefixes(guild_id).await;

        if !prefixes.is_empty() {
            let prefix = prefixes.swap_remove(0);

            if prefix == "<" && prefixes.len() == 1 {
                (None, prefix)
            } else {
                let prefix_iter = prefixes.iter();
                let mut prefixes_str = String::with_capacity(9);
                let _ = write!(prefixes_str, "`{prefix}`");

                for prefix in prefix_iter {
                    let _ = write!(prefixes_str, ", `{prefix}`");
                }

                (Some(prefixes_str), prefix)
            }
        } else {
            (None, "<".into())
        }
    } else {
        (None, "<".into())
    };

    let prefix_desc = custom_prefix.map_or_else(
        || String::from("Prefix: `<` (none required in DMs)"),
        |p| format!("Server prefix: {p}\nDM prefix: `<` or none at all"),
    );

    format!(
        ":fire: **Slash commands now supported!** Type `/` to check them out :fire:\n\n\
        {prefix_desc}\n\
        __**General**__\n\
        - To find out more about a command like what arguments you can give or which shorter aliases \
        it has,  use __**`{first_prefix}help [command]`**__, e.g. `{first_prefix}help simulate`.
        - If you want to specify an argument, e.g. a username, that contains \
        spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
        - If you've used the `/link` command to connect to an osu! account, \
        you can omit the username for any command that needs one.\n\
        - If you have questions, complains, or suggestions for the bot, feel free to join its \
        [discord server]({BATHBOT_WORKSHOP}) and let Badewanne3 know.\n\
        [This roadmap]({BATHBOT_ROADMAP}) shows already suggested features and known bugs.\n\
        __**Mods for osu!**__
        Many commands allow you to specify mods. You can do so with `+mods` \
        for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. For example:\n\
        `+hdhr`: scores that include at least HD and HR\n\
        `+hd!`: only HD scores\n\
        `-nm!`: scores that are not NoMod\n\
        `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ"
    )
}

async fn dm_help(ctx: Arc<Context>, msg: &Message) -> Result<()> {
    let owner = msg.author.id;

    let channel = match ctx.http.create_private_channel(owner).exec().await {
        Ok(channel_res) => channel_res.model().await?.id,
        Err(err) => {
            let content = "Your DMs seem blocked :(\n\
            Perhaps you disabled incoming messages from other server members?";
            let report = Report::new(err).wrap_err("Failed to create DM channel");
            warn!("{report:?}");
            msg.error(&ctx, content).await?;

            return Ok(());
        }
    };

    if msg.guild_id.is_some() {
        let content = "Don't mind me sliding into your DMs :eyes:";
        let builder = MessageBuilder::new().embed(content);
        let _ = msg.create_message(&ctx, &builder).await;
    }

    let desc = description(&ctx, msg.guild_id).await;
    let embed = EmbedBuilder::new().description(desc).build();
    let components = help_select_menu(None);
    let builder = MessageBuilder::new().embed(embed).components(components);

    if let Err(err) = channel.create_message(&ctx, &builder).await {
        let report = Report::new(err).wrap_err("Failed to send help chunk");
        warn!("{report:?}");
        let content = "Could not DM you, perhaps you disabled it?";
        msg.error(&ctx, content).await?;
    }

    Ok(())
}

pub async fn handle_help_category(
    ctx: &Context,
    mut component: InteractionComponent,
) -> Result<()> {
    let value = component.data.values.pop().wrap_err("missing menu value")?;

    let group = match value.as_str() {
        "general" => {
            let desc = description(ctx, None).await;
            let embed = EmbedBuilder::new().description(desc).build();
            let components = help_select_menu(None);
            let builder = MessageBuilder::new().embed(embed).components(components);

            component.callback(ctx, builder).await?;

            return Ok(());
        }
        "osu" => PrefixCommandGroup::Osu,
        "taiko" => PrefixCommandGroup::Taiko,
        "ctb" => PrefixCommandGroup::Catch,
        "mania" => PrefixCommandGroup::Mania,
        "all_modes" => PrefixCommandGroup::AllModes,
        "tracking" => PrefixCommandGroup::Tracking,
        "twitch" => PrefixCommandGroup::Twitch,
        "games" => PrefixCommandGroup::Games,
        "utility" => PrefixCommandGroup::Utility,
        "songs" => PrefixCommandGroup::Songs,
        _ => bail!("got unexpected value `{value}`"),
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

    let embed = EmbedBuilder::new().description(desc).footer(footer).build();
    let components = help_select_menu(Some(group));
    let builder = MessageBuilder::new().embed(embed).components(components);

    component.callback(ctx, builder).await?;

    Ok(())
}

fn help_select_menu(default: Option<PrefixCommandGroup>) -> Vec<Component> {
    let options = vec![
        SelectMenuOption {
            default: matches!(default, None),
            description: None,
            emoji: Some(ReactionType::Unicode {
                name: "🛁".to_owned(),
            }),
            label: "General".to_owned(),
            value: "general".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Osu)),
            description: None,
            emoji: Some(Emote::Std.reaction_type()),
            label: "osu!".to_owned(),
            value: "osu".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Taiko)),
            description: None,
            emoji: Some(Emote::Tko.reaction_type()),
            label: "Taiko".to_owned(),
            value: "taiko".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Catch)),
            description: None,
            emoji: Some(Emote::Ctb.reaction_type()),
            label: "Catch".to_owned(),
            value: "ctb".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Mania)),
            description: None,
            emoji: Some(Emote::Mna.reaction_type()),
            label: "Mania".to_owned(),
            value: "mania".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::AllModes)),
            description: None,
            emoji: Some(Emote::Osu.reaction_type()),
            label: "All Modes".to_owned(),
            value: "all_modes".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Tracking)),
            description: None,
            emoji: Some(Emote::Tracking.reaction_type()),
            label: "Tracking".to_owned(),
            value: "tracking".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Twitch)),
            description: None,
            emoji: Some(Emote::Twitch.reaction_type()),
            label: "Twitch".to_owned(),
            value: "twitch".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Games)),
            description: None,
            emoji: Some(ReactionType::Unicode {
                name: "🎮".to_owned(),
            }),
            label: "Games".to_owned(),
            value: "games".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Utility)),
            description: None,
            emoji: Some(ReactionType::Unicode {
                name: "🛠️".to_owned(),
            }),
            label: "Utility".to_owned(),
            value: "utility".to_owned(),
        },
        SelectMenuOption {
            default: matches!(default, Some(PrefixCommandGroup::Songs)),
            description: None,
            emoji: Some(ReactionType::Unicode {
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
        options,
        placeholder: None,
    };

    let category_row = ActionRow {
        components: vec![Component::SelectMenu(category)],
    };

    vec![Component::ActionRow(category_row)]
}
