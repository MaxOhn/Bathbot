use std::{collections::{BTreeMap, HashSet}, fmt::Write};

use bathbot_macros::command;
use bathbot_psql::model::configs::GuildConfig;
use bathbot_util::{
    string_cmp::levenshtein_distance, AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder,
};
use eyre::Result;
use twilight_model::{
    channel::message::{embed::EmbedField, Message},
    guild::Permissions,
};

use super::failed_message_content;
use crate::{
    active::{impls::HelpPrefixMenu, ActiveMessageOriginError, ActiveMessages},
    core::{
        commands::prefix::{PrefixCommand, PrefixCommands},
        Context,
    },
    util::ChannelExt,
};

#[command]
#[desc("Display help for prefix commands")]
#[group(Utility)]
#[alias("h")]
#[usage("[command]")]
#[example("", "recent", "osg")]
async fn prefix_help(
    msg: &Message,
    mut args: Args<'_>,
    permissions: Option<Permissions>,
) -> Result<()> {
    match args.next() {
        Some(arg) => match PrefixCommands::get().command(arg) {
            Some(cmd) => command_help(msg, cmd, permissions).await,
            None => failed_help(msg, arg).await,
        },
        None => dm_help(msg, permissions).await,
    }
}

async fn failed_help(msg: &Message, name: &str) -> Result<()> {
    let mut seen = HashSet::new();

    let dists: BTreeMap<_, _> = PrefixCommands::get()
        .iter()
        .filter(|cmd| seen.insert(cmd.name()))
        .flat_map(|cmd| cmd.names.iter())
        .map(|&cmd| (levenshtein_distance(name, cmd).0, cmd))
        .filter(|(dist, _)| *dist < 4)
        .collect();

    let content = failed_message_content(dists);
    msg.error(content).await?;

    Ok(())
}

async fn command_help(
    msg: &Message,
    cmd: &PrefixCommand,
    permissions: Option<Permissions>,
) -> Result<()> {
    let name = cmd.name();

    let guild_config = match msg.guild_id {
        Some(guild_id) => Some(
            Context::guild_config()
                .peek(guild_id, GuildConfig::to_owned)
                .await,
        ),
        None => None,
    };

    let prefix = guild_config
        .as_ref()
        .and_then(|config| config.prefixes.first().cloned())
        .unwrap_or_else(|| GuildConfig::DEFAULT_PREFIX.to_owned());

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
        let value = if let Some(config) = guild_config {
            let authorities = config.authorities;

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

    let embed = eb.footer(footer).fields(fields);
    let builder = MessageBuilder::new().embed(embed);

    msg.create_message(builder, permissions).await?;

    Ok(())
}

async fn dm_help(msg: &Message, permissions: Option<Permissions>) -> Result<()> {
    let owner = msg.author.id;

    let channel = match Context::http().create_private_channel(owner).await {
        Ok(channel_res) => channel_res.model().await?.id,
        Err(err) => {
            let content = "Your DMs seem blocked :(\n\
            Perhaps you disabled incoming messages from other server members?";
            warn!(?err, "Failed to create DM channel");
            msg.error(content).await?;

            return Ok(());
        }
    };

    if msg.guild_id.is_some() {
        let content = "Don't mind me sliding into your DMs :eyes:";
        let builder = MessageBuilder::new().embed(content);
        let _ = msg.create_message(builder, permissions).await;
    }

    let help_menu = HelpPrefixMenu::new(msg.guild_id);
    let active_fut = ActiveMessages::builder(help_menu).begin_with_err(channel);

    match active_fut.await {
        Ok(_) => Ok(()),
        Err(ActiveMessageOriginError::Report(err)) => Err(err),
        Err(ActiveMessageOriginError::CannotDmUser) => {
            let content = "Could not DM you, perhaps you disabled it?";
            msg.error(content).await?;

            Ok(())
        }
    }
}
