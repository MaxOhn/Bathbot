use std::{
    borrow::Cow,
    cmp::{Ordering, Reverse},
    fmt::Write,
};

use bathbot_macros::command;
use bathbot_model::{ArchivedOsekaiMedal, MedalGroup, OsekaiComment, OsekaiMap};
use bathbot_psql::model::configs::HideSolutions;
use bathbot_util::{
    AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder, MessageBuilder,
    constants::{FIELD_VALUE_SIZE, GENERAL_ISSUE, OSEKAI_ISSUE, OSU_BASE},
    fields,
    osu::flag_url,
    string_cmp::levenshtein_similarity,
};
use eyre::{Report, Result, WrapErr};
use rkyv::{rend::f32_le, vec::ArchivedVec};
use rosu_v2::prelude::GameMode;
use time::OffsetDateTime;
use twilight_interactions::command::AutocompleteValue;
use twilight_model::{
    application::command::{CommandOptionChoice, CommandOptionChoiceValue},
    channel::message::embed::EmbedField,
};

use super::{MedalAchieved, MedalInfo_};
use crate::{
    Context,
    core::commands::CommandOrigin,
    util::{ChannelExt, InteractionCommandExt, interaction::InteractionCommand},
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
async fn prefix_medal(msg: &Message, args: Args<'_>) -> Result<()> {
    let name = args.rest().trim_matches('"');

    if name.is_empty() {
        msg.error("You must specify a medal name").await?;

        return Ok(());
    }

    let args = MedalInfo_ {
        name: AutocompleteValue::Completed(name.into()),
    };

    info(msg.into(), args).await
}

pub(super) async fn info(orig: CommandOrigin<'_>, args: MedalInfo_<'_>) -> Result<()> {
    let MedalInfo_ { name } = args;

    let medals = match Context::redis().medals().await {
        Ok(medals) => medals,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get cached medals"));
        }
    };

    let name = match (name, &orig) {
        (AutocompleteValue::None, CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(command, String::new()).await;
        }
        (AutocompleteValue::Focused(name), CommandOrigin::Interaction { command }) => {
            return handle_autocomplete(command, name).await;
        }
        (AutocompleteValue::Completed(name), _) => name,
        _ => unreachable!(),
    };

    let name = name.cow_to_ascii_lowercase();

    let medal = match medals
        .iter()
        .position(|m| m.name.to_ascii_lowercase() == name)
    {
        Some(idx) => &medals[idx],
        None => return no_medal(&orig, name.as_ref(), &medals).await,
    };

    let client = Context::client();
    let map_fut = client.get_osekai_beatmaps(medal.medal_id.to_native());
    let comment_fut = client.get_osekai_comments(medal.medal_id.to_native());

    let (mut maps, comments) = match tokio::try_join!(map_fut, comment_fut) {
        Ok((maps, comments)) => (maps, comments),
        Err(err) => {
            let _ = orig.error(OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get osekai map or comments"));
        }
    };

    let top_comment = comments
        .into_iter()
        .max_by_key(|comment| comment.vote_count)
        .filter(|comment| comment.vote_count > 0);

    // Remove all dups
    maps.sort_unstable_by_key(|map| Reverse(map.map_id));
    maps.dedup_by_key(|map| map.map_id);

    maps.sort_unstable_by_key(|map| Reverse(map.vote_count));

    let hide_solution = match orig.guild_id() {
        Some(guild) => {
            Context::guild_config()
                .peek(guild, |config| {
                    config.hide_medal_solution.unwrap_or(HideSolutions::ShowAll)
                })
                .await
        }
        None => HideSolutions::ShowAll,
    };

    let embed_data = MedalEmbed::new(medal, None, maps, top_comment, hide_solution);
    let embed = embed_data.finish();
    let builder = MessageBuilder::new().embed(embed);
    orig.create_message(builder).await?;

    Ok(())
}

const SIMILARITY_THRESHOLD: f32 = 0.6;

async fn no_medal(
    orig: &CommandOrigin<'_>,
    name: &str,
    medals: &ArchivedVec<ArchivedOsekaiMedal>,
) -> Result<()> {
    let mut medals: Vec<_> = medals
        .iter()
        .map(|medal| {
            let medal = medal.name.to_ascii_lowercase();

            (levenshtein_similarity(name, &medal), medal)
        })
        .collect();

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

    orig.error(content).await
}

pub async fn handle_autocomplete(command: &InteractionCommand, name: String) -> Result<()> {
    let name = if name.is_empty() {
        command.autocomplete(Vec::new()).await?;

        return Ok(());
    } else {
        name.cow_to_ascii_lowercase()
    };

    let name = name.as_ref();

    let medals = Context::redis()
        .medals()
        .await
        .wrap_err("Failed to get cached medals")?;

    let mut choices = Vec::with_capacity(25);

    for medal in medals.iter() {
        if medal.name.to_ascii_lowercase().starts_with(name) {
            choices.push(new_choice(&medal.name));

            if choices.len() == 25 {
                break;
            }
        }
    }

    command.autocomplete(choices).await?;

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

const SPOILER: &str = "||";

impl MedalEmbed {
    pub fn new(
        medal: &ArchivedOsekaiMedal,
        achieved: Option<MedalAchieved<'_>>,
        maps: Vec<OsekaiMap>,
        comment: Option<OsekaiComment>,
        hide_solution: HideSolutions,
    ) -> Self {
        let as_spoiler = match hide_solution {
            HideSolutions::ShowAll => false,
            HideSolutions::HideHushHush => matches!(
                medal.grouping,
                MedalGroup::HushHush | MedalGroup::HushHushExpert
            ),
            HideSolutions::HideAll => true,
        };

        let solution = medal
            .solution()
            .filter(|s| !s.is_empty())
            .unwrap_or(Cow::Borrowed("Not yet solved                      "));

        let solution = if as_spoiler {
            format!("{SPOILER}{solution}{SPOILER}")
        } else {
            solution.into_owned()
        };

        let mut mode_mods = String::with_capacity(16);

        if as_spoiler {
            mode_mods.push_str(SPOILER);
        }

        if medal.mode.is_none() && medal.mods.is_none() {
            // Padded to not make the potential spoiler too obvious
            mode_mods.push_str("Any      ");
        } else {
            if let Some(mode) = medal.mode.as_ref() {
                let _ = write!(mode_mods, "{mode}");
            } else {
                mode_mods.push_str("Any");
            }

            mode_mods.push_str(" • ");

            if let Some(mods) = medal.mods.as_deref() {
                let _ = write!(mode_mods, "{mods}");
            } else {
                mode_mods.push_str("Any");
            }
        }

        if as_spoiler {
            mode_mods.push_str(SPOILER);
        }

        let rarity = medal
            .rarity
            .as_ref()
            .copied()
            .map_or(0.0, f32_le::to_native);

        let mut availability = String::new();

        if as_spoiler {
            availability.push_str(SPOILER);
        }

        availability.push_str(match (medal.supports_lazer, medal.supports_stable) {
            (true, true) => "Lazer & Stable",
            (true, false) => "Lazer-only",
            (false, true) => "Stable-only",
            (false, false) => "Neither lazer nor stable",
        });

        if as_spoiler {
            availability.push_str(SPOILER);
        }

        let mut fields = fields![
            "Description", medal.description.as_ref().to_owned(), false;
            "Group", medal.grouping.to_string(), true;
            "Rarity", format!("{rarity:.2}%"), true;
            "Solution", solution, false;
            "Mode • Mods", mode_mods, true;
            "Availability", availability, true;
        ];

        if !(maps.is_empty() || as_spoiler) {
            let len = maps.len();
            let mut map_value = String::with_capacity(256);
            let mut map_buf = String::new();

            for map in maps {
                let OsekaiMap {
                    title,
                    version,
                    map_id,
                    vote_count: vote_sum,
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

        if let Some(comment) = comment.filter(|_| !as_spoiler) {
            let OsekaiComment {
                content,
                username,
                vote_count: vote_sum,
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
        let thumbnail = medal.icon_url().to_string();

        let url = match medal.url() {
            Ok(url) => url,
            Err(err) => {
                warn!(?err);

                medal.backup_url()
            }
        };

        let achieved = achieved.map(|achieved| {
            let user = achieved.user;

            let country_code = user.country_code.as_str();
            let username = user.username.as_str();
            let user_id = user.user_id.to_native();

            let mut author_url = format!("{OSU_BASE}users/{user_id}");

            match medal.mode.as_ref() {
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

    pub fn finish(self) -> EmbedBuilder {
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
