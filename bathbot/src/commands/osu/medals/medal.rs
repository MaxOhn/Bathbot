use std::{
    cmp::{Ordering, Reverse},
    fmt::Write,
    sync::Arc,
};

use bathbot_macros::command;
use bathbot_model::{OsekaiComment, OsekaiMap, OsekaiMedal};
use bathbot_util::{
    constants::{FIELD_VALUE_SIZE, OSEKAI_ISSUE, OSU_BASE},
    fields,
    osu::flag_url,
    string_cmp::levenshtein_similarity,
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageBuilder,
};
use eyre::{Result, WrapErr};
use rkyv::{Deserialize, Infallible};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_interactions::command::AutocompleteValue;
use twilight_model::{
    application::command::{CommandOptionChoice, CommandOptionChoiceValue},
    channel::message::embed::EmbedField,
};

use super::{MedalAchieved, MedalInfo_};
use crate::{
    core::commands::CommandOrigin,
    manager::redis::RedisData,
    util::{interaction::InteractionCommand, ChannelExt, InteractionCommandExt},
    Context,
};

#[command]
#[desc("Display info about an osu! medal")]
#[help(
    "Display info about an osu! medal.\n\
    The given name must be exact (but case-insensitive).\n\
    All data originates from [osekai](https://osekai.net/medals/), \
    check it out for more info."
)]
#[usage("[medal name]")]
#[examples(r#""50,000 plays""#, "any%")]
#[group(AllModes)]
async fn prefix_medal(ctx: Arc<Context>, msg: &Message, args: Args<'_>) -> Result<()> {
    let name = args.rest().trim_matches('"');

    if name.is_empty() {
        msg.error(&ctx, "You must specify a medal name").await?;

        return Ok(());
    }

    let args = MedalInfo_ {
        name: AutocompleteValue::Completed(name.into()),
    };

    info(ctx, msg.into(), args).await
}

pub(super) async fn info(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: MedalInfo_<'_>,
) -> Result<()> {
    let MedalInfo_ { name } = args;

    let mut medals = match ctx.redis().medals().await {
        Ok(medals) => medals,
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medals"));
        }
    };

    let name = match (name, &orig) {
        (AutocompleteValue::None, CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(&ctx, command, String::new()).await
        }
        (AutocompleteValue::Focused(name), CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(&ctx, command, name).await
        }
        (AutocompleteValue::Completed(name), _) => name,
        _ => unreachable!(),
    };

    let name = name.cow_to_ascii_lowercase();

    let medal = match medals {
        RedisData::Original(ref mut original) => match original
            .iter()
            .position(|m| m.name.to_ascii_lowercase() == name)
        {
            Some(idx) => original.swap_remove(idx),
            None => return no_medal(&ctx, &orig, name.as_ref(), medals).await,
        },
        RedisData::Archive(ref archived) => {
            match archived
                .iter()
                .position(|m| m.name.to_ascii_lowercase() == name)
            {
                Some(idx) => archived[idx].deserialize(&mut Infallible).unwrap(),
                None => return no_medal(&ctx, &orig, name.as_ref(), medals).await,
            }
        }
    };

    let map_fut = ctx.client().get_osekai_beatmaps(&medal.name);
    let comment_fut = ctx.client().get_osekai_comments(medal.medal_id);

    let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(err) => {
            let _ = orig.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get osekai map or comments"));
        }
    };

    let top_comment = comments
        .into_iter()
        .filter(|comment| comment.parent_id == 0)
        .max_by_key(|comment| comment.vote_sum)
        .filter(|comment| comment.vote_sum > 0);

    // Remove all dups
    maps.sort_unstable_by_key(|map| Reverse(map.map_id));
    maps.dedup_by_key(|map| map.map_id);

    maps.sort_unstable_by_key(|map| Reverse(map.vote_sum));

    let embed_data = MedalEmbed::new(&medal, None, maps, top_comment);
    let embed = embed_data.maximized();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(&ctx, builder).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.6;

async fn no_medal(
    ctx: &Context,
    orig: &CommandOrigin<'_>,
    name: &str,
    medals: RedisData<Vec<OsekaiMedal>>,
) -> Result<()> {
    let mut medals: Vec<_> = match medals {
        RedisData::Original(original) => original
            .iter()
            .map(|medal| {
                let medal = medal.name.to_ascii_lowercase();

                (levenshtein_similarity(name, &medal), medal)
            })
            .collect(),
        RedisData::Archive(archived) => archived
            .iter()
            .map(|medal| {
                let medal = medal.name.to_ascii_lowercase();

                (levenshtein_similarity(name, &medal), medal)
            })
            .collect(),
    };

    medals.sort_unstable_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    let mut content = format!("No medal found with the name `{name}`.");

    let mut iter = medals
        .into_iter()
        .take(5)
        .take_while(|(similarity, _)| *similarity >= SIMILARITY_THRESHOLD);

    if let Some((_, first)) = iter.next() {
        let _ = write!(content, "\nDid you mean `{first}`");

        for (_, medal) in iter {
            let _ = write!(content, ", `{medal}`");
        }

        content.push('?');
    }

    orig.error(ctx, content).await
}

pub async fn handle_autocomplete(
    ctx: &Context,
    command: &InteractionCommand,
    name: String,
) -> Result<()> {
    let name = if name.is_empty() {
        command.autocomplete(ctx, Vec::new()).await?;

        return Ok(());
    } else {
        name.cow_to_ascii_lowercase()
    };

    let name = name.as_ref();

    let medals = ctx
        .redis()
        .medals()
        .await
        .wrap_err("Failed to get cached medals")?;

    let mut choices = Vec::with_capacity(25);

    match medals {
        RedisData::Original(original) => {
            for medal in original.iter() {
                if medal.name.to_ascii_lowercase().starts_with(name) {
                    choices.push(new_choice(&medal.name));

                    if choices.len() == 25 {
                        break;
                    }
                }
            }
        }
        RedisData::Archive(archived) => {
            for medal in archived.iter() {
                if medal.name.to_ascii_lowercase().starts_with(name) {
                    choices.push(new_choice(&medal.name));

                    if choices.len() == 25 {
                        break;
                    }
                }
            }
        }
    }

    command.autocomplete(ctx, choices).await?;

    Ok(())
}

fn new_choice(name: &str) -> CommandOptionChoice {
    CommandOptionChoice {
        name: name.to_owned(),
        name_localizations: None,
        value: CommandOptionChoiceValue::String(name.to_owned()),
    }
}

#[derive(Clone)]
pub struct MedalEmbed {
    achieved: Option<(AuthorBuilder, FooterBuilder, OffsetDateTime)>,
    fields: Vec<EmbedField>,
    thumbnail: String,
    title: String,
    url: String,
}

impl MedalEmbed {
    pub fn new(
        medal: &OsekaiMedal,
        achieved: Option<MedalAchieved<'_>>,
        maps: Vec<OsekaiMap>,
        comment: Option<OsekaiComment>,
    ) -> Self {
        let mut fields = Vec::with_capacity(7);

        fields![fields { "Description", medal.description.as_ref().to_owned(), false }];

        if let Some(solution) = medal.solution().filter(|s| !s.is_empty()) {
            fields![fields { "Solution", solution.into_owned(), false }];
        }

        let mode_mods = match (medal.restriction, medal.mods.as_deref()) {
            (None, None) => "Any".to_owned(),
            (None, Some(mods)) => format!("Any • {mods}"),
            (Some(mode), None) => format!("{mode} • Any"),
            (Some(mode), Some(mods)) => format!("{mode} • {mods}"),
        };

        fields![fields {
            "Rarity", format!("{:.2}%", medal.rarity), true;
            "Mode • Mods", mode_mods, true;
            "Group", medal.grouping.to_string(), true;
        }];

        if !maps.is_empty() {
            let len = maps.len();
            let mut map_value = String::with_capacity(256);
            let mut map_buf = String::new();

            for map in maps {
                let OsekaiMap {
                    title,
                    version,
                    map_id,
                    vote_sum,
                    ..
                } = map;

                map_buf.clear();

                let _ = writeln!(
                    map_buf,
                    "- [{title} [{version}]]({OSU_BASE}b/{map_id}) ({vote_sum:+})",
                    title = title.cow_escape_markdown(),
                    version = version.cow_escape_markdown()
                );

                if map_buf.len() + map_value.len() + 7 >= FIELD_VALUE_SIZE {
                    map_value.push_str("`...`\n");

                    break;
                } else {
                    map_value.push_str(&map_buf);
                }
            }

            map_value.pop();

            fields![fields { format!("Beatmaps: {len}"), map_value, false }];
        }

        if let Some(comment) = comment {
            let OsekaiComment {
                content,
                username,
                vote_sum,
                ..
            } = comment;

            let value = format!(
                "```\n\
                {content}\n    \
                - {username} [{vote_sum:+}]\n\
                ```",
                content = content.trim(),
            );

            fields![fields { "Top comment", value, false }];
        }

        let title = medal.name.as_ref().to_owned();
        let thumbnail = medal.icon_url.as_ref().to_owned();

        let url = format!(
            "https://osekai.net/medals/?medal={}",
            title.cow_replace(' ', "+").cow_replace(',', "%2C")
        );

        let achieved = achieved.map(|achieved| {
            let user = achieved.user;

            let (country_code, username, user_id) = match &user {
                RedisData::Original(user) => {
                    let country_code = user.country_code.as_str();
                    let username = user.username.as_str();
                    let user_id = user.user_id;

                    (country_code, username, user_id)
                }
                RedisData::Archive(user) => {
                    let country_code = user.country_code.as_str();
                    let username = user.username.as_str();
                    let user_id = user.user_id;

                    (country_code, username, user_id)
                }
            };

            let mut author_url = format!("{OSU_BASE}users/{user_id}");

            match medal.restriction {
                None => {}
                Some(GameMode::Osu) => author_url.push_str("/osu"),
                Some(GameMode::Taiko) => author_url.push_str("/taiko"),
                Some(GameMode::Catch) => author_url.push_str("/fruits"),
                Some(GameMode::Mania) => author_url.push_str("/mania"),
            }

            let author = AuthorBuilder::new(username)
                .url(author_url)
                .icon_url(flag_url(country_code));

            let footer = FooterBuilder::new(format!(
                "Medal {}/{} | Achieved",
                achieved.index + 1,
                achieved.medal_count
            ));

            (author, footer, achieved.achieved_at)
        });

        Self {
            achieved,
            fields,
            thumbnail,
            title,
            url,
        }
    }

    pub fn minimized(mut self) -> EmbedBuilder {
        self.fields.truncate(5);

        self.maximized()
    }

    pub fn maximized(self) -> EmbedBuilder {
        let builder = EmbedBuilder::new()
            .fields(self.fields)
            .thumbnail(self.thumbnail)
            .title(self.title)
            .url(self.url);

        match self.achieved {
            Some((author, footer, timestamp)) => {
                builder.author(author).footer(footer).timestamp(timestamp)
            }
            None => builder,
        }
    }
}
