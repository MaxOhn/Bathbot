use std::{collections::BTreeMap, fmt::Write, sync::Arc, time::Duration};

use command_macros::command;
use tokio::time::{interval, MissedTickBehavior};
use twilight_model::{
    channel::{embed::EmbedField, Message},
    id::{marker::GuildMarker, Id},
};

use crate::{
    core::{
        commands::prefix::{PrefixCommand, PrefixCommandGroup, PREFIX_COMMANDS},
        Context,
    },
    util::{
        builder::{AuthorBuilder, EmbedBuilder, FooterBuilder, MessageBuilder},
        constants::{BATHBOT_WORKSHOP, DESCRIPTION_SIZE, OWNER_USER_ID},
        levenshtein_distance, ChannelExt,
    },
    BotResult,
};

use super::failed_message_;

#[command]
#[desc("Display help for prefix commands")]
#[group(Utility)]
#[alias("h")]
#[usage("[command]")]
#[example("", "recent", "osg")]
async fn prefix_help(ctx: Arc<Context>, msg: &Message, mut args: Args<'_>) -> BotResult<()> {
    match args.next() {
        Some(arg) => match PREFIX_COMMANDS.command(arg) {
            Some(cmd) => command_help(ctx, msg, cmd).await,
            None => failed_help(ctx, msg, arg).await,
        },
        None => dm_help(ctx, msg).await,
    }
}

async fn failed_help(ctx: Arc<Context>, msg: &Message, name: &str) -> BotResult<()> {
    let dists: BTreeMap<_, _> = PREFIX_COMMANDS
        .collect()
        .into_iter()
        .map(|cmd| (levenshtein_distance(name, cmd.name).0, cmd.name))
        .filter(|(dist, _)| *dist < 3)
        .collect();

    failed_message_(&ctx, msg.channel_id, dists).await
}

async fn command_help(ctx: Arc<Context>, msg: &Message, cmd: &PrefixCommand) -> BotResult<()> {
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

    format!(":fire: **Slash commands now supported!** Type `/` to check them out :fire:\n\n\
        {prefix_desc}\n\
        __**General**__\n\
        - To find out more about a command like what arguments you can give or which shorter aliases it has, \
        use __**`{first_prefix}help [command]`**__, e.g. `{first_prefix}help simulate`.
        - If you want to specify an argument, e.g. a username, that contains \
        spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
        - If you've used the `/link` command to connect to an osu! account, you can omit the username for any command that needs one.\n\
        - With the arrow reactions you can scroll through pages e.g. check an earlier play than the most recent one. \
        Note that generally only reactions of the response invoker (user who used command) will be processed.\n\
        - ~~`Strikethrough`~~ commands indicate that either you can't use them in DMs or \
        you lack authority status in the server.\n\
        - If you have questions, complains, or suggestions for the bot, feel free to join its \
        [discord server]({BATHBOT_WORKSHOP}) and let Badewanne3 know.\n\
        __**Mods for osu!**__
        Many commands allow you to specify mods. You can do so with `+mods` \
        for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. For example:\n\
        `+hdhr`: scores that include at least HD and HR\n\
        `+hd!`: only HD scores\n\
        `-nm!`: scores that are not NoMod\n\
        `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ\n\
        \n__**All commands:**__\n")
}

macro_rules! send_chunk {
    ($ctx:ident, $msg:ident, $content:ident, $interval:ident) => {
        let embed = EmbedBuilder::new().description($content).build();
        let builder = MessageBuilder::new().embed(embed);
        $interval.tick().await;

        if let Err(err) = $msg.create_message(&$ctx, &builder).await {
            // let report = Report::new(why).wrap_err("error while sending help chunk");
            // warn!("{:?}", report);
            let content = "Could not DM you, perhaps you disabled it?";
            $msg.error(&$ctx, content).await?;

            return Ok(());
        }
    };
}

async fn dm_help(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    let owner = msg.author.id;

    // TODO: Gather info, maybe concurrent to private channel?
    let is_authority = true;

    let channel = match ctx.http.create_private_channel(owner).exec().await {
        Ok(channel_res) => channel_res.model().await?.id,
        Err(err) => {
            let content = "Your DMs seem blocked :(\n\
            Perhaps you disabled incoming messages from other server members?";
            // let report = Report::new(err).wrap_err("error while creating DM channel");
            // warn!("{:?}", report);

            msg.error(&ctx, content).await?;

            return Ok(());
        }
    };

    if msg.guild_id.is_some() {
        let content = "Don't mind me sliding into your DMs :eyes:";
        let builder = MessageBuilder::new().embed(content);
        let _ = msg.create_message(&ctx, &builder).await;
    }

    let mut buf = description(&ctx, msg.guild_id).await;

    let mut curr_group = PrefixCommandGroup::AllModes;

    let _ = writeln!(
        buf,
        "\n{} __**{}**__",
        PrefixCommandGroup::AllModes.emote(),
        PrefixCommandGroup::AllModes.name(),
    );

    let mut size = buf.len();
    let mut next_size;

    debug_assert!(
        size < DESCRIPTION_SIZE,
        "description size {size} > {DESCRIPTION_SIZE}",
    );

    let mut cmds = PREFIX_COMMANDS.collect();

    if owner.get() != OWNER_USER_ID {
        cmds.retain(|c| c.group != PrefixCommandGroup::Owner);
    }

    cmds.sort_by_key(|cmd| cmd.group);
    cmds.dedup_by_key(|cmd| cmd.name());

    let mut interval = interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    for cmd in cmds {
        // Next group
        if cmd.group != curr_group {
            curr_group = cmd.group;
            let emote = cmd.group.emote();
            let name = cmd.group.name();

            next_size = emote.len() + name.len() + 11;

            if size + next_size > DESCRIPTION_SIZE {
                send_chunk!(ctx, channel, buf, interval);
                buf = String::with_capacity(DESCRIPTION_SIZE);
                size = 0;
            }

            size += next_size;
            let _ = writeln!(buf, "\n{emote} __**{name}**__");
        }

        let name = cmd.name();

        next_size = (cmd.flags.authority()) as usize * 4 + 5 + name.len() + cmd.desc.len();

        if size + next_size > DESCRIPTION_SIZE {
            send_chunk!(ctx, channel, buf, interval);
            buf = String::with_capacity(DESCRIPTION_SIZE);
            size = 0;
        }

        size += next_size;

        let _ = writeln!(
            buf,
            "{strikethrough}`{name}`{strikethrough}: {}",
            cmd.desc,
            strikethrough = if cmd.flags.authority() && !is_authority {
                "~~"
            } else {
                ""
            }
        );
    }

    if !buf.is_empty() {
        send_chunk!(ctx, channel, buf, interval);
    }

    Ok(())
}
