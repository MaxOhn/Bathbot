use std::fmt;

use command_macros::EmbedData;
use rosu_v2::model::user::User;
use time::{Duration, OffsetDateTime};
use twilight_model::channel::embed::EmbedField;

use crate::util::{
    self, builder::AuthorBuilder, constants::OSU_BASE, datetime::DATE_FORMAT,
    numbers::with_comma_int, osu::flag_url, CowUtils,
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

        if let Some(last_visit) = user.last_visit {
            let field = EmbedField {
                inline: true,
                name: "Last active".to_owned(),
                value: last_visit.format(DATE_FORMAT).unwrap(),
            };

            fields.push(field);
        }

        if let Some(ref stats) = user.statistics {
            let field = EmbedField {
                inline: true,
                name: "Total playcount".to_owned(),
                value: with_comma_int(stats.playcount).to_string(),
            };

            fields.push(field);
        }

        let name = name.cow_to_ascii_lowercase();
        let is_prev_name = user.username.cow_to_ascii_lowercase() != name;

        let field = if user.badges.as_ref().map_or(0, Vec::len) > 0 {
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
        } else if user.ranked_mapset_count.unwrap_or(0) > 0 {
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
        } else if let Some(duration) = time_to_wait(user) {
            let date = OffsetDateTime::now_utc() + duration;

            let name = if duration.is_negative() {
                "Name available since"
            } else {
                "Name available at"
            };

            let value = format!(
                "{}{}",
                date.format(DATE_FORMAT).unwrap(),
                TimeUntil(duration),
            );

            EmbedField {
                inline: false,
                name: name.to_owned(),
                value,
            }
        } else {
            available_at_field("Last visit date unavailable, cannot calculate :(")
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

fn time_to_wait(user: &User) -> Option<Duration> {
    let last_seen = user.last_visit?;
    let inactive_time = OffsetDateTime::now_utc() - last_seen;

    let x = match user.statistics {
        Some(ref stats) if stats.playcount > 0 => stats.playcount as f32,
        _ => return Some(Duration::days(6 * 30) - inactive_time),
    };

    const I: f32 = 180.0;
    const S: f32 = 5900.0;
    const H: f32 = 1580.0;
    const B: f32 = 8.0;

    let extra_days = H * (1.0 - (-x / S).exp()) + I + B * x / S;

    Some(Duration::days(extra_days as i64) - inactive_time)
}

#[derive(Copy, Clone)]
struct TimeUntil(Duration);

impl fmt::Display for TimeUntil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut minutes = self.0.whole_minutes();

        if minutes < 0 {
            return Ok(());
        } else if minutes < 60 {
            return f.write_str(" (any minute now)");
        }

        f.write_str(" (")?;

        let years = minutes / (60 * 24 * 365);
        minutes -= years * (60 * 24 * 365);

        let months = minutes / (60 * 24 * 30);
        minutes -= months * (60 * 24 * 30);

        let days = minutes / (60 * 24);
        minutes -= days * (60 * 24);

        if years + months + days > 0 {
            if years > 0 {
                write!(f, "{years}y")?;
            }

            if months > 0 {
                if years > 0 {
                    f.write_str(" ")?;
                }

                write!(f, "{months}m")?;
            }

            if days > 0 {
                if years + months > 0 {
                    f.write_str(" ")?;
                }

                write!(f, "{days}d")?;
            }
        } else {
            let hours = minutes / 60;
            minutes -= hours * 60;

            f.write_str("~")?;

            if hours > 0 {
                write!(f, "{hours}h")?;
            }

            if minutes > 0 {
                if hours > 0 {
                    f.write_str(" ")?;
                }

                write!(f, "{minutes}m")?;
            }
        }

        f.write_str(")")
    }
}
