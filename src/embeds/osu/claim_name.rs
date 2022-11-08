use command_macros::EmbedData;
use time::{Duration, OffsetDateTime};
use twilight_model::channel::embed::EmbedField;

use crate::{
    manager::redis::osu::User,
    util::{
        self, builder::AuthorBuilder, constants::OSU_BASE, numbers::WithComma, osu::flag_url,
        CowUtils,
    },
};

#[derive(EmbedData)]
pub struct ClaimNameEmbed {
    author: AuthorBuilder,
    thumbnail: String,
    fields: Vec<EmbedField>,
}

impl ClaimNameEmbed {
    pub fn new(user: &User, name: &str) -> Self {
        let mut fields = Vec::with_capacity(3);

        let last_visit = EmbedField {
            inline: true,
            name: "Last active".to_owned(),
            value: user.last_visit.map_or_else(
                || "Date not available :(".to_owned(),
                |datetime| format!("<t:{}:d>", datetime.unix_timestamp()),
            ),
        };

        fields.push(last_visit);

        if let Some(ref stats) = user.statistics {
            let field = EmbedField {
                inline: true,
                name: "Total playcount".to_owned(),
                value: WithComma::new(stats.playcount).to_string(),
            };

            fields.push(field);
        }

        let name = name.cow_to_ascii_lowercase();
        let is_prev_name = user.username.cow_to_ascii_lowercase() != name;

        let field = if !user.badges.is_empty() {
            let value = if is_prev_name {
                format!(
                    "{} has a different name now but they have badges \
                    so `{name}` likely won't be released in the future",
                    user.username
                )
            } else {
                format!(
                    "{} has badges so that name likely won't be released in the future",
                    user.username
                )
            };

            available_at_field(value)
        } else if user.ranked_mapset_count > 0 {
            let value = if is_prev_name {
                format!(
                    "{} has a different name now but they have ranked maps \
                    so `{name}` likely won't be released in the future",
                    user.username
                )
            } else {
                format!(
                    "{} has ranked maps so that name likely won't be released in the future",
                    user.username
                )
            };

            available_at_field(value)
        } else if util::contains_disallowed_infix(name.as_ref()) {
            let value = format!("`{name}` likely won't be accepted as name in the future");

            available_at_field(value)
        } else if is_prev_name {
            let value = format!(
                "{} has a different name now so `{name}` should be available",
                user.username
            );

            available_at_field(value)
        } else {
            let duration = time_to_wait(user);
            let date = OffsetDateTime::now_utc() + duration;

            let name = if duration.is_negative() {
                "Name available since"
            } else {
                "Name available at"
            };

            let value = format!(
                "{preamble}<t:{timestamp}:d> (<t:{timestamp}:R>)",
                preamble = if user.last_visit.is_none() {
                    "Assuming the user is inactive from now on:\n"
                } else {
                    ""
                },
                timestamp = date.unix_timestamp(),
            );

            EmbedField {
                inline: false,
                name: name.to_owned(),
                value,
            }
        };

        fields.push(field);

        let author = AuthorBuilder::new(user.username.to_string())
            .url(format!("{OSU_BASE}u/{}", user.user_id))
            .icon_url(flag_url(user.country_code.as_str()));

        Self {
            author,
            thumbnail: user.avatar_url.to_owned(),
            fields,
        }
    }
}

fn available_at_field(value: impl Into<String>) -> EmbedField {
    EmbedField {
        inline: false,
        name: "Name available at".to_owned(),
        value: value.into(),
    }
}

fn time_to_wait(user: &User) -> Duration {
    let inactive_time = user.last_visit.map_or(Duration::ZERO, |last_seen| {
        OffsetDateTime::now_utc() - last_seen
    });

    let x = match user.statistics {
        Some(ref stats) if stats.playcount > 0 => stats.playcount as f32,
        _ => return Duration::days(6 * 30) - inactive_time,
    };

    const I: f32 = 180.0;
    const S: f32 = 5900.0;
    const H: f32 = 1580.0;
    const B: f32 = 8.0;

    let extra_days = H * (1.0 - (-x / S).exp()) + I + B * x / S;

    Duration::days(extra_days as i64) - inactive_time
}
