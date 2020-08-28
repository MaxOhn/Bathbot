use crate::{
    core::{Command, CommandGroups},
    util::{
        constants::{
            DARK_GREEN, DESCRIPTION_SIZE, EMBED_SIZE, FIELD_VALUE_SIZE, OWNER_USER_ID, RED, BATHBOT_WORKSHOP
        },
        content_safe, levenshtein_distance, MessageExt,
    },
    BotResult, Context,
};

use rayon::prelude::*;
use std::{collections::BTreeMap, fmt::Write};
use twilight::model::{
    channel::{
        embed::{Embed, EmbedField},
        Message,
    },
    id::{ChannelId, GuildId, UserId},
};
use twilight_embed_builder::{
    author::EmbedAuthorBuilder, builder::EmbedBuilder, footer::EmbedFooterBuilder,
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
        just pass the command as argument e.g. __**`{prefix}help globalleaderboard`**__.\n\
        - If you want to specify an argument, e.g. a username, that contains \
        spaces, you must encapsulate it with `\"` i.e. `\"nathan on osu\"`.\n\
        - If you used `{prefix}link osuname`, you can ommit the osu username for any command that needs one.\n\
        - If you react with :x: within one minute to my response, I will delete it.\n\
        - With reactions like :track_previous: or :fast_forward: you can scroll through pages \
        e.g. check an earlier play than the most recent one
        - ~~`Strikethrough`~~ commands indicate that you lack authority status in the server.\n\
        - If you have questions, complains, or suggestions for the bot, feel free to join its \
        [discord server]({discord_url}) and let Badewanne3 know.
        __**Mods for osu!**__
        Many commands allow you to specify mods. You can do so with `+mods` \
        for included mods, `+mods!` for exact mods, or `-mods!` for excluded mods. For example:\n\
        `+hdhr`: scores that include at least HD and HR\n\
        `+hd!`: only HD scores\n\
        `-nm!`: scores that are not NoMod\n\
        `-nfsohdez!`: scores that have neither NF, SO, HD, or EZ\n\
        \n__**These are all commands:**__", prefix_desc, prefix = first_prefix, discord_url = BATHBOT_WORKSHOP)
}

pub async fn help(
    ctx: &Context,
    cmds: &CommandGroups,
    msg: &Message,
    is_authority: bool,
) -> BotResult<()> {
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
    let mut eb = EmbedBuilder::new()
        .color(DARK_GREEN)
        .unwrap()
        .description(desc)?;
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
        // No owner check so be sure owner commands are in the owner group
        for &cmd in &group.commands {
            if cmd.authority && !is_authority {
                write!(value, "~~`{}`~~", cmd.names[0])?;
            } else {
                write!(value, "`{}`", cmd.names[0])?;
            }
            writeln!(value, ": {}", cmd.short_desc)?;
        }
        debug_assert!(value.len() < FIELD_VALUE_SIZE);
        let size_addition = group.name.len() + value.len();
        debug_assert!(size_addition < EMBED_SIZE);
        eb = if size + size_addition > EMBED_SIZE {
            if let Err(why) = send_help_chunk(ctx, channel.id, owner, eb.build()?).await {
                warn!("Error while sending help chunk: {}", why);
                let content = "Could not DM you, have you disabled it?";
                return msg.error(ctx, content).await;
            }
            size = 0;
            EmbedBuilder::new().color(DARK_GREEN).unwrap()
        } else {
            size += size_addition;
            eb.field(EmbedField {
                name: group.name.clone(),
                value,
                inline: false,
            })
        };
    }
    let embed = eb.build()?;
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
        .color(DARK_GREEN)?
        .title(name)?
        .description(cmd.long_desc.unwrap_or(cmd.short_desc))?;
    let mut usage_len = 0;
    if let Some(usage) = cmd.usage {
        let value = format!("`{}{} {}`", prefix, name, usage);
        usage_len = value.chars().count();
        let field = EmbedField {
            name: String::from("How to use"),
            value,
            inline: usage_len <= 29,
        };
        eb = eb.field(field);
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
            || ((usage_len > 29 || cmd.names.len() > 1) && example_len > 36);
        let field = EmbedField {
            name: String::from("Examples"),
            value,
            inline: !not_inline,
        };
        eb = eb.field(field);
    }
    let mut aliases = cmd.names.iter().skip(1);
    if let Some(first) = aliases.next() {
        let len: usize = cmd.names.iter().skip(1).map(|n| 4 + n.len()).sum();
        let mut value = String::with_capacity(len);
        write!(value, "`{}`", first)?;
        for &alias in aliases {
            write!(value, ", `{}`", alias)?;
        }
        eb = eb.field(EmbedField {
            name: String::from("Aliases"),
            value,
            inline: true,
        });
    }
    if cmd.authority {
        let value = if let Some(guild_id) = msg.guild_id {
            let authorities = ctx.config_authorities(guild_id);
            let mut value = "You need admin permission".to_owned();
            let mut iter = authorities.iter();
            if let Some(first) = iter.next() {
                let _ = write!(value, " or any of these roles: <@&{}>", first);
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
        eb = eb.field(EmbedField {
            name: String::from("Requires authority status"),
            value,
            inline: false,
        });
    }
    if cmd.owner {
        let ab = EmbedAuthorBuilder::new()
            .name("Can only be used by the bot owner")
            .unwrap();
        eb = eb.author(ab);
    }
    let footer_text = if cmd.only_guilds || cmd.authority {
        "Only available in guilds"
    } else {
        "Available in guilds and DMs"
    };
    let fb = EmbedFooterBuilder::new(footer_text).unwrap();
    let embed = eb.footer(fb).build()?;
    msg.build_response(ctx, |m| m.embed(embed)).await?;
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
    let embed = EmbedBuilder::new()
        .description(content)?
        .color(color)?
        .build()?;
    msg.build_response(ctx, |m| m.embed(embed)).await?;
    Ok(())
}
