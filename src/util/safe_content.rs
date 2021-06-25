use crate::Context;

use std::str::FromStr;
use twilight_model::id::{ChannelId, GuildId, RoleId, UserId};

pub fn content_safe(ctx: &Context, content: &mut String, guild_id: Option<GuildId>) {
    clean_roles(ctx, content);
    clean_channels(ctx, content);
    clean_users(ctx, content, guild_id);

    *content = content.replace("@here", "@\u{200B}here");
    *content = content.replace("@everyone", "@\u{200B}everyone");
}

fn clean_roles(ctx: &Context, s: &mut String) {
    let mut progress = 0;

    while let Some(mut mention_start) = s[progress..].find("<@&") {
        mention_start += progress;

        if let Some(mut mention_end) = s[mention_start..].find('>') {
            mention_end += mention_start;
            mention_start += "<@&".len();

            if let Ok(id) = u64::from_str(&s[mention_start..mention_end]) {
                let to_replace = format!("<@&{}>", id);

                if let Some(role) = ctx.cache.role(RoleId(id)) {
                    *s = s.replace(&to_replace, &format!("@{}", &role.name))
                } else {
                    *s = s.replace(&to_replace, &"@deleted-role")
                }
            } else {
                let id = &s[mention_start..mention_end].to_string();

                if !id.is_empty() && id.as_bytes().iter().all(u8::is_ascii_digit) {
                    let to_replace = format!("<@&{}>", id);
                    *s = s.replace(&to_replace, &"@deleted-role");
                } else {
                    progress = mention_end;
                }
            }
        } else {
            break;
        }
    }
}

fn clean_channels(ctx: &Context, s: &mut String) {
    let mut progress = 0;

    while let Some(mut mention_start) = s[progress..].find("<#") {
        mention_start += progress;

        if let Some(mut mention_end) = s[mention_start..].find('>') {
            mention_end += mention_start;
            mention_start += "<#".len();

            if let Ok(id) = u64::from_str(&s[mention_start..mention_end]) {
                let to_replace = format!("<#{}>", &s[mention_start..mention_end]);

                if let Some(channel) = ctx.cache.guild_channel(ChannelId(id)) {
                    let replacement = format!("#{}", channel.name());
                    *s = s.replace(&to_replace, &replacement)
                } else {
                    *s = s.replace(&to_replace, &"#deleted-channel")
                }
            } else {
                let id = &s[mention_start..mention_end].to_string();

                if !id.is_empty() && id.as_bytes().iter().all(u8::is_ascii_digit) {
                    let to_replace = format!("<#{}>", id);
                    *s = s.replace(&to_replace, &"#deleted-channel");
                } else {
                    progress = mention_end;
                }
            }
        } else {
            break;
        }
    }
}

fn clean_users(ctx: &Context, s: &mut String, guild: Option<GuildId>) {
    let mut progress = 0;

    while let Some(mut mention_start) = s[progress..].find("<@") {
        mention_start += progress;

        if let Some(mut mention_end) = s[mention_start..].find('>') {
            mention_end += mention_start;
            mention_start += 2; // "<@".len()

            let has_exclamation = if s[mention_start..]
                .as_bytes()
                .get(0)
                .map_or(false, |c| *c == b'!')
            {
                mention_start += 1; // "!".len()

                true
            } else {
                false
            };

            if let Ok(id) = u64::from_str(&s[mention_start..mention_end]) {
                let user = ctx.cache.user(UserId(id));

                let replacement = if let Some(guild_id) = guild {
                    match (ctx.cache.member(guild_id, UserId(id)), user) {
                        (Some(member), Some(user)) => {
                            format!(
                                "@{}#{:04}",
                                member.nick.as_deref().unwrap_or(&user.name),
                                user.discriminator
                            )
                        }
                        _ => "@invalid-user".to_string(),
                    }
                } else if let Some(user) = user {
                    format!("@{}#{:04}", user.name, user.discriminator)
                } else {
                    "@invalid-user".to_string()
                };

                let code_start = if has_exclamation { "<@!" } else { "<@" };
                let to_replace = format!("{}{}>", code_start, &s[mention_start..mention_end]);
                *s = s.replace(&to_replace, &replacement)
            } else {
                let id = &s[mention_start..mention_end].to_string();

                if !id.is_empty() && id.as_bytes().iter().all(u8::is_ascii_digit) {
                    let code_start = if has_exclamation { "<@!" } else { "<@" };
                    let to_replace = format!("{}{}>", code_start, id);
                    *s = s.replace(&to_replace, &"@invalid-user");
                } else {
                    progress = mention_end;
                }
            }
        } else {
            break;
        }
    }
}
