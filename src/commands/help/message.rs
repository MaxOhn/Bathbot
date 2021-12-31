use std::{collections::BTreeMap, fmt::Write, time::Duration};

use eyre::Report;
use tokio::time::sleep;
use twilight_model::{channel::embed::EmbedField, id::GuildId};

use crate::{
    core::{
        commands::{CommandData, CommandDataCompact, CMD_GROUPS},
        Command as CoreCommand, Context,
    },
    embeds::{Author, EmbedBuilder, Footer},
    util::{
        constants::{BATHBOT_WORKSHOP, DESCRIPTION_SIZE, GENERAL_ISSUE, OWNER_USER_ID},
        levenshtein_distance, MessageBuilder, MessageExt,
    },
    BotResult,
};

use super::failed_message_;

async fn description(ctx: &Context, guild_id: Option<GuildId>) -> String {
    let (custom_prefix, first_prefix) = if let Some(guild_id) = guild_id {
        let mut prefixes = ctx.guild_prefixes(guild_id).await;

        if !prefixes.is_empty() {
            let prefix = prefixes.swap_remove(0);

            if prefix == "<" && prefixes.len() == 1 {
                (None, prefix)
            } else {
                let prefix_iter = prefixes.iter();
                let mut prefixes_str = String::with_capacity(9);
                let _ = write!(prefixes_str, "`{}`", prefix);

                for prefix in prefix_iter {
                    let _ = write!(prefixes_str, ", `{}`", prefix);
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
        |p| format!("Server prefix: {}\nDM prefix: `<` or none at all", p),
    );

    format!(":fire: **Slash commands now supported!** Type `/` to check them out :fire:\n\n\
        {prefix_desc}\n\
        __**General**__\n\
        - To find out more about a command like what arguments you can give or which shorter aliases it has, \
        use __**`{prefix}help [command]`**__, e.g. `{prefix}help simulate`.
        - If you want to specify an argument, e.g. a username, that contains \
        spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
        - If you've used the `/link` command to connect to an osu! account, you can omit the username for any command that needs one.\n\
        - With the arrow reactions you can scroll through pages e.g. check an earlier play than the most recent one. \
        Note that generally only reactions of the response invoker (user who used command) will be processed.\n\
        - ~~`Strikethrough`~~ commands indicate that either you can't use them in DMs or \
        you lack authority status in the server.\n\
        - If you have questions, complains, or suggestions for the bot, feel free to join its \
        [discord server]({discord_url}) and let Badewanne3 know.\n\
        __**Mods for osu!**__
        Many commands allow you to specify mods. You can do so with `+mods` \
        for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. For example:\n\
        `+hdhr`: scores that include at least HD and HR\n\
        `+hd!`: only HD scores\n\
        `-nm!`: scores that are not NoMod\n\
        `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ\n\
        \n__**All commands:**__\n", prefix_desc = prefix_desc, prefix = first_prefix, discord_url = BATHBOT_WORKSHOP)
}

pub async fn help(ctx: &Context, data: CommandData<'_>, is_authority: bool) -> BotResult<()> {
    let owner = data.author()?.id;

    let channel_id = match ctx.http.create_private_channel(owner).exec().await {
        Ok(channel_res) => match channel_res.model().await {
            Ok(channel) => channel.id,
            Err(why) => {
                let _ = data.error(ctx, GENERAL_ISSUE).await;

                return Err(why.into());
            }
        },
        Err(err) => {
            let content = "Your DMs seem blocked :(\n\
            Perhaps you disabled incoming messages from other server members?";
            let report = Report::new(err).wrap_err("error while creating DM channel");
            warn!("{:?}", report);

            return data.error(ctx, content).await;
        }
    };

    let guild_id = data.guild_id();

    if guild_id.is_some() {
        let content = "Don't mind me sliding into your DMs :eyes:";
        let builder = MessageBuilder::new().embed(content);
        let _ = data.create_message(ctx, builder).await;
    }

    let mut buf = description(ctx, guild_id).await;
    let mut size = buf.len();

    debug_assert!(
        size < DESCRIPTION_SIZE,
        "description size {} > {}",
        size,
        DESCRIPTION_SIZE,
    );

    let groups = CMD_GROUPS
        .groups
        .iter()
        .filter(|g| owner.get() == OWNER_USER_ID || g.name != "owner");

    let mut next_size;

    for group in groups {
        let emote = group.emote.text();

        next_size = emote.len() + group.name.len() + 11;

        if size + next_size > DESCRIPTION_SIZE {
            let embed = &[EmbedBuilder::new().description(buf).build()];
            let msg_fut = ctx.http.create_message(channel_id).embeds(embed)?;

            if let Err(why) = msg_fut.exec().await {
                let report = Report::new(why).wrap_err("error while sending help chunk");
                warn!("{:?}", report);
                let content = "Could not DM you, perhaps you disabled it?";

                return data.error(ctx, content).await;
            }

            sleep(Duration::from_millis(50)).await;
            buf = String::with_capacity(DESCRIPTION_SIZE);
            size = 0;
        }

        size += next_size;
        let _ = writeln!(buf, "\n{} __**{}**__", emote, group.name);

        for &cmd in group.commands.iter() {
            next_size =
                (cmd.authority) as usize * 4 + 5 + cmd.names[0].len() + cmd.short_desc.len();

            if size + next_size > DESCRIPTION_SIZE {
                let embed = &[EmbedBuilder::new().description(buf).build()];
                let msg_fut = ctx.http.create_message(channel_id).embeds(embed)?;

                if let Err(why) = msg_fut.exec().await {
                    let report = Report::new(why).wrap_err("error while sending help chunk");
                    warn!("{:?}", report);
                    let content = "Could not DM you, perhaps you disabled it?";

                    return data.error(ctx, content).await;
                }

                sleep(Duration::from_millis(50)).await;
                buf = String::with_capacity(DESCRIPTION_SIZE);
                size = 0;
            }

            size += next_size;

            let _ = writeln!(
                buf,
                "{strikethrough}`{}`{strikethrough}: {}",
                cmd.names[0],
                cmd.short_desc,
                strikethrough = if cmd.authority && !is_authority {
                    "~~"
                } else {
                    ""
                }
            );
        }
    }

    if !buf.is_empty() {
        let embed = &[EmbedBuilder::new().description(buf).build()];
        let msg_fut = ctx.http.create_message(channel_id).embeds(embed)?;

        if let Err(why) = msg_fut.exec().await {
            let report = Report::new(why).wrap_err("error while sending help chunk");
            warn!("{:?}", report);
            let content = "Could not DM you, perhaps you disabled it?";

            return data.error(ctx, content).await;
        }
    }

    Ok(())
}

pub async fn help_command(
    ctx: &Context,
    cmd: &CoreCommand,
    guild_id: Option<GuildId>,
    data: CommandDataCompact,
) -> BotResult<()> {
    let name = cmd.names[0];
    let prefix = ctx.guild_first_prefix(guild_id).await;
    let mut fields = Vec::new();

    let mut eb = EmbedBuilder::new()
        .title(name)
        .description(cmd.long_desc.unwrap_or(cmd.short_desc));

    let mut usage_len = 0;

    if let Some(usage) = cmd.usage {
        let value = format!("`{}{} {}`", prefix, name, usage);
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
        writeln!(value, "`{}{} {}`", prefix, name, first)?;

        for example in examples {
            writeln!(value, "`{}{} {}`", prefix, name, example)?;
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
        write!(value, "`{}`", first)?;

        for &alias in aliases {
            write!(value, ", `{}`", alias)?;
        }

        let field = EmbedField {
            name: "Aliases".to_owned(),
            value,
            inline: true,
        };

        fields.push(field);
    }

    if cmd.authority {
        let value = if let Some(guild_id) = guild_id {
            let authorities = ctx.guild_authorities(guild_id).await;
            let mut value = "You need admin permission".to_owned();
            let mut iter = authorities.iter();

            if let Some(first) = iter.next() {
                let _ = write!(value, " or any of these roles: <@&{}>", first);

                for role in iter {
                    let _ = write!(value, ", <@&{}>", role);
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

    if cmd.owner {
        let author = Author::new("Can only be used by the bot owner");
        eb = eb.author(author);
    }

    let footer_text = if cmd.only_guilds || cmd.authority {
        "Only available in servers"
    } else {
        "Available in servers and DMs"
    };

    let footer = Footer::new(footer_text);

    let embed = eb.footer(footer).fields(fields).build();
    let builder = MessageBuilder::new().embed(embed);

    data.create_message(ctx, builder).await?;

    Ok(())
}

pub async fn failed_help(ctx: &Context, arg: &str, data: CommandDataCompact) -> BotResult<()> {
    let dists: BTreeMap<_, _> = CMD_GROUPS
        .groups
        .iter()
        .flat_map(|group| group.commands.iter().flat_map(|&cmd| cmd.names))
        .copied()
        .map(|name| (levenshtein_distance(arg, name).0, name))
        .filter(|(dist, _)| *dist < 3)
        .collect();

    failed_message_(ctx, data, dists).await
}
