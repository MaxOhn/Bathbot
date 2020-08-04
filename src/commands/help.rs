use crate::{
    core::{Command, CommandGroups},
    util::{
        constants::{
            DARK_GREEN, DESCRIPTION_SIZE, EMBED_SIZE, FIELD_VALUE_SIZE, OWNER_USER_ID, RED,
        },
        content_safe, levenshtein_distance, MessageExt,
    },
    BotResult, Context,
};

use rayon::prelude::*;
use std::{collections::BTreeMap, fmt::Write};
use twilight::builders::embed::EmbedBuilder;
use twilight::model::{
    channel::{embed::Embed, Message},
    id::{ChannelId, GuildId, UserId},
};

fn description(ctx: &Context, guild_id: Option<GuildId>) -> String {
    let (custom_prefix, first_prefix) = if let Some(guild_id) = guild_id {
        let mut prefixes = ctx.config_prefixes(guild_id);
        if prefixes == ["<"] {
            (None, prefixes.remove(0))
        } else {
            let mut prefix_iter = prefixes.iter();
            let mut prefixes_str = String::with_capacity(9);
            let _ = write!(prefixes_str, "`{}`", prefix_iter.next().unwrap());
            for prefix in prefix_iter {
                let _ = write!(prefixes_str, ", `{}`", prefix);
            }
            (Some(prefixes_str), prefixes.remove(0))
        }
    } else {
        (None, "<".to_string())
    };
    let prefix_desc = custom_prefix.map_or_else(
        || String::from("Prefix: `<` (none required in DMs)"),
        |p| format!("Server prefix: {}\nDM prefix: `<` or none at all", p),
    );
    format!("{}\n__**General**__\n\
        - Most commands have (shorter) aliases, e.g. `{prefix}glb` instead of `{prefix}globalleaderboard`. \
        To check those out or get more info about a command in general, \
        just pass the command as argument i.e. __**`{prefix}help command`**__.\n\
        - If you want to specify an argument, e.g. a username, that contains \
        spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
        - If you used `{prefix}link osuname`, you can ommit the osu username for any command that needs one.\n\
        - If you react with :x: to my response, I will delete it.\n\
        __**Mods for osu!**__
        Many commands allow you to specify mods. You can do so with `+mods` \
        for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. For example:\n\
        `-nm!`: scores that are not NoMod\n\
        `+hdhr`: scores that include at least HD and HR\n\
        `+hd!`: only HD scores\n\
        `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ\n\
        \n__**These are all commands:**__", prefix_desc, prefix = first_prefix)
}

pub async fn help(ctx: &Context, cmds: &CommandGroups, msg: &Message) -> BotResult<()> {
    let channel = match ctx.http.create_private_channel(msg.author.id).await {
        Ok(channel) => channel,
        Err(why) => {
            let content = "Your DMs seem blocked :(\n\
               Did you disable messages from other guild members?";
            debug!("Error while creating DM channel: {}", why);
            return msg.error(&ctx, content).await;
        }
    };
    let owner = msg.author.id;
    if msg.guild_id.is_some() {
        let content = "Don't mind me sliding into your DMs :eyes:";
        let _ = msg.reply(ctx, content).await;
    }
    let desc = description(ctx, msg.guild_id);
    let mut size = desc.len();
    debug_assert!(size < DESCRIPTION_SIZE);
    let mut eb = EmbedBuilder::new().color(DARK_GREEN).description(desc);
    let groups = cmds
        .groups
        .iter()
        .filter(|g| owner.0 == OWNER_USER_ID || g.name != "owner");
    for group in groups {
        let len: usize = group
            .commands
            .iter()
            .map(|&c| c.names[0].len() + 5 + c.short_desc.len())
            .sum();
        let mut value = String::with_capacity(len);
        for &cmd in &group.commands {
            writeln!(value, "`{}`: {}", cmd.names[0], cmd.short_desc)?;
        }
        debug_assert!(value.len() < FIELD_VALUE_SIZE);
        let size_addition = group.name.len() + value.len();
        debug_assert!(size_addition < EMBED_SIZE);
        eb = if size + size_addition > EMBED_SIZE {
            if let Err(why) = send_help_chunk(ctx, channel.id, owner, eb.build()).await {
                warn!("Error while sending help chunk: {}", why);
                let content = "Could not DM you, have you disabled it?";
                return msg.error(ctx, content).await;
            }
            size = 0;
            EmbedBuilder::new().color(DARK_GREEN)
        } else {
            size += size_addition;
            eb.add_field(&group.name, value).commit()
        };
    }
    let embed = eb.build();
    if !embed.fields.is_empty() {
        if let Err(why) = send_help_chunk(ctx, channel.id, owner, embed).await {
            warn!("Error while sending help chunk: {}", why);
            let content = "Could not DM you, have you disabled it?";
            return msg.error(ctx, content).await;
        }
    }
    Ok(())
}

async fn send_help_chunk(
    ctx: &Context,
    channel: ChannelId,
    owner: UserId,
    embed: Embed,
) -> BotResult<()> {
    ctx.http
        .create_message(channel)
        .embed(embed)?
        .await?
        .reaction_delete(ctx, owner);
    Ok(())
}

pub async fn help_command(ctx: &Context, cmd: &Command, msg: &Message) -> BotResult<()> {
    let name = cmd.names[0];
    let prefix = ctx.config_first_prefix(msg.guild_id);
    let mut eb = EmbedBuilder::new()
        .color(DARK_GREEN)
        .title(name)
        .description(cmd.long_desc.unwrap_or(cmd.short_desc));
    let mut usage_len = 0;
    if let Some(usage) = cmd.usage {
        let value = format!("`{}{} {}`", prefix, name, usage);
        usage_len = value.chars().count();
        let fb = eb.add_field("How to use", value);
        eb = if usage_len > 29 {
            fb.commit()
        } else {
            fb.inline().commit()
        };
    }
    if !cmd.examples.is_empty() {
        let len: usize = cmd.examples.iter().map(|&e| name.len() + e.len() + 4).sum();
        let mut value = String::with_capacity(len);
        let mut examples = cmd.examples.iter();
        let mut example_len = 0;
        let cmd_len = prefix.chars().count() + name.chars().count();
        writeln!(value, "`{}{} {}`", prefix, name, examples.next().unwrap())?;
        for example in examples {
            writeln!(value, "`{}{} {}`", prefix, name, example)?;
            example_len = example_len.max(cmd_len + example.chars().count());
        }
        let fb = eb.add_field("Examples", value);
        eb = if (usage_len <= 29 && cmd.names.len() > 1 && example_len > 27)
            || ((usage_len > 29 || cmd.names.len() > 1) && example_len > 36)
        {
            fb.commit()
        } else {
            fb.inline().commit()
        };
    }
    if cmd.names.len() > 1 {
        let len: usize = cmd.names.iter().skip(1).map(|n| 4 + n.len()).sum();
        let mut value = String::with_capacity(len);
        let mut aliases = cmd.names.iter().skip(1);
        write!(value, "`{}`", aliases.next().unwrap())?;
        for &alias in aliases {
            write!(value, ", `{}`", alias)?;
        }
        eb = eb.add_field("Aliases", value).inline().commit();
    }
    if cmd.authority {
        let value = if let Some(guild_id) = msg.guild_id {
            let authorities = ctx.config_authorities(guild_id);
            let mut value = "You need admin permission".to_owned();
            if !authorities.is_empty() {
                let mut iter = authorities.iter();
                let _ = write!(
                    value,
                    " or any of these roles: <@&{}>",
                    iter.next().unwrap()
                );
                for role in iter {
                    let _ = write!(value, ", <@&{}>", role);
                }
            }
            content_safe(&ctx, &mut value, msg.guild_id);
            value
        } else {
            "Admin permission or any role that \
            was setup as authority in a guild"
                .to_owned()
        };
        eb = eb.add_field("Requires authority status", value).commit();
    }
    let footer_text = if cmd.only_guilds || cmd.authority {
        "Only available in guilds"
    } else {
        "Available in guilds and DMs"
    };
    eb = eb.footer(footer_text).commit();
    msg.build_response(ctx, |m| m.embed(eb.build())).await?;
    Ok(())
}

pub async fn failed_help(
    ctx: &Context,
    arg: &str,
    cmds: &CommandGroups,
    msg: &Message,
) -> BotResult<()> {
    let dists: BTreeMap<_, _> = cmds
        .groups
        .par_iter()
        .flat_map(|group| group.commands.par_iter().flat_map(|&cmd| cmd.names))
        .map(|name| (levenshtein_distance(arg, name), name))
        .filter(|(dist, _)| *dist < 4)
        .collect();
    let (content, color) = if dists.is_empty() {
        (String::from("There is no such command"), RED)
    } else {
        let mut names = dists.iter().take(5).map(|(_, &name)| name);
        let count = dists.len().min(5);
        let mut content = String::with_capacity(14 + count * (4 + 2) + (count - 1) * 2);
        content.push_str("Did you mean ");
        write!(content, "`{}`", names.next().unwrap())?;
        for name in names {
            write!(content, ", `{}`", name)?;
        }
        content.push('?');
        (content, DARK_GREEN)
    };
    let eb = EmbedBuilder::new().description(content).color(color);
    msg.build_response(ctx, |m| m.embed(eb.build())).await?;
    Ok(())
}
