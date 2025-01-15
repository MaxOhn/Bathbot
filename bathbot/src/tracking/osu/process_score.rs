use std::{slice, sync::Arc};

use bathbot_model::{
    embed_builder::{
        ComboValue, HitresultsValue, ScoreEmbedSettings, SettingValue, SettingsButtons,
        SettingsImage, Value,
    },
    rosu_v2::user::User,
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{constants::UNKNOWN_CHANNEL, EmbedBuilder};
use rosu_v2::{model::GameMode, prelude::Score};
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType as TwilightErrorType,
};
use twilight_model::id::Id;

use super::{entry::TrackEntry, OsuTracking};
use crate::{
    active::impls::{MarkIndex, SingleScoreContent, SingleScorePagination},
    commands::utility::ScoreEmbedDataWrap,
    core::{BotMetrics, Context},
    manager::{
        redis::{
            osu::{UserArgs, UserArgsSlim},
            RedisData,
        },
        OsuMap,
    },
};

pub async fn process_score(score: Score, entry: Arc<TrackEntry>) {
    let Some(pp) = score.pp else { return };

    let user_args = UserArgsSlim::user_id(score.user_id).mode(score.mode);
    let user_fut = Context::redis().osu_user(UserArgs::Args(user_args));
    let tops_fut = Context::osu_scores().top(false).limit(100).exec(user_args);

    let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());
    let map_fut = Context::osu_map().map(score.map_id, checksum);

    let (user, tops, map) = match tokio::join!(user_fut, tops_fut, map_fut) {
        (Ok(user), Ok(scores), Ok(map)) => (user, scores, map),
        (Err(err), ..) | (_, Err(err), _) => {
            warn!(
                user_id = score.user_id,
                mode = ?score.mode,
                ?err,
                "Failed to get user or top scores for tracking"
            );

            return;
        }
        (.., Err(err)) => {
            warn!(
                map_id = score.map_id,
                ?err,
                "Failed to get map for tracking"
            );

            return;
        }
    };

    entry.insert_last_pp(score.user_id, score.mode, &tops).await;

    let Some(idx) = tops.iter().position(|s| s.id == score.id) else {
        return;
    };

    BotMetrics::osu_tracking_hit(score.mode);

    let combo = score.max_combo;
    let (builder, max_combo) = embed_builder(&user, score, map, idx).await;
    let idx = idx as u8 + 1;
    let embed = builder.build();
    let embeds = slice::from_ref(&embed);
    let combo_percent = max_combo.map(|max| 100.0 * combo as f32 / max as f32);

    let http = Context::http();
    let guard = entry.guard_channels();

    for (channel_id, params) in entry.iter_channels(&guard) {
        if !params.matches(idx, pp, combo_percent) {
            continue;
        }

        let channel = Id::new(channel_id.get());

        let err = match http.create_message(channel).embeds(embeds) {
            Ok(msg_fut) => match msg_fut.await {
                Ok(_) => continue,
                Err(err) => err,
            },
            Err(err) => {
                warn!(?err, "Invalid embed for osu notif");

                break;
            }
        };

        let TwilightErrorType::Response { error, .. } = err.kind() else {
            warn!(%channel, ?err, "Error while sending osu notif");

            continue;
        };

        let ApiError::General(GeneralApiError {
            code: UNKNOWN_CHANNEL,
            ..
        }) = error
        else {
            warn!(%channel, ?error, "Error from API while sending osu notif");

            continue;
        };

        OsuTracking::remove_channel(channel, None).await;
    }
}

async fn embed_builder(
    user: &RedisData<User>,
    score: Score,
    map: OsuMap,
    idx: usize,
) -> (EmbedBuilder, Option<u32>) {
    let settings = match score.mode {
        GameMode::Mania => create_mania_settings(),
        _ => create_settings(),
    };

    let score_data = ScoreData::Lazer;
    let msg_owner = Id::new(1);
    let content = SingleScoreContent::None;

    let embed_data = ScoreEmbedDataWrap::new_custom(score, map, idx, None).await;

    // This is always `Some` considering `ScoreEmbedDataWrap::new_custom`
    // creates *full* data but let's map regardless to be extra sure.
    let max_combo = embed_data.try_get().map(|data| data.max_combo);

    let entries = Box::<[_]>::from([embed_data]);

    let mut pagination =
        SingleScorePagination::new(user, entries, settings, score_data, msg_owner, content);

    let build_fut = pagination.async_build_page(Box::default(), MarkIndex::Skip);

    match build_fut.await {
        Ok(data) => (data.into_embed(), max_combo),
        // Unreachable because `async_build_page` can only fail while
        // converting to full score data but it already starts off as
        // full.
        Err(_) => Default::default(),
    }
}

fn create_settings() -> ScoreEmbedSettings {
    ScoreEmbedSettings {
        values: vec![
            SettingValue {
                inner: Value::Grade,
                y: 0,
            },
            SettingValue {
                inner: Value::Mods,
                y: 0,
            },
            SettingValue {
                inner: Value::Score,
                y: 0,
            },
            SettingValue {
                inner: Value::Accuracy,
                y: 0,
            },
            SettingValue {
                inner: Value::Combo(Default::default()),
                y: 0,
            },
            SettingValue {
                inner: Value::Pp(Default::default()),
                y: 1,
            },
            SettingValue {
                inner: Value::Hitresults(HitresultsValue::Full),
                y: 1,
            },
            SettingValue {
                inner: Value::Length,
                y: 2,
            },
            SettingValue {
                inner: Value::Cs,
                y: 2,
            },
            SettingValue {
                inner: Value::Ar,
                y: 2,
            },
            SettingValue {
                inner: Value::Od,
                y: 2,
            },
            SettingValue {
                inner: Value::Hp,
                y: 2,
            },
            SettingValue {
                inner: Value::Bpm(Default::default()),
                y: 2,
            },
            SettingValue {
                inner: Value::Mapper(Default::default()),
                y: SettingValue::FOOTER_Y,
            },
            SettingValue {
                inner: Value::ScoreDate,
                y: SettingValue::FOOTER_Y,
            },
        ],
        show_artist: true,
        show_sr_in_title: true,
        image: SettingsImage::Thumbnail,
        buttons: SettingsButtons {
            pagination: false,
            render: false,
            miss_analyzer: false,
        },
    }
}

fn create_mania_settings() -> ScoreEmbedSettings {
    ScoreEmbedSettings {
        values: vec![
            SettingValue {
                inner: Value::Grade,
                y: 0,
            },
            SettingValue {
                inner: Value::Mods,
                y: 0,
            },
            SettingValue {
                inner: Value::Score,
                y: 0,
            },
            SettingValue {
                inner: Value::Accuracy,
                y: 0,
            },
            SettingValue {
                inner: Value::Combo(ComboValue { max: false }),
                y: 0,
            },
            SettingValue {
                inner: Value::Ratio,
                y: 0,
            },
            SettingValue {
                inner: Value::Pp(Default::default()),
                y: 1,
            },
            SettingValue {
                inner: Value::Hitresults(HitresultsValue::Full),
                y: 1,
            },
            SettingValue {
                inner: Value::Length,
                y: 2,
            },
            SettingValue {
                inner: Value::Od,
                y: 2,
            },
            SettingValue {
                inner: Value::Hp,
                y: 2,
            },
            SettingValue {
                inner: Value::Bpm(Default::default()),
                y: 2,
            },
            SettingValue {
                inner: Value::CountSliders(Default::default()),
                y: 2,
            },
            SettingValue {
                inner: Value::Mapper(Default::default()),
                y: SettingValue::FOOTER_Y,
            },
            SettingValue {
                inner: Value::ScoreDate,
                y: SettingValue::FOOTER_Y,
            },
        ],
        show_artist: true,
        show_sr_in_title: true,
        image: SettingsImage::Thumbnail,
        buttons: SettingsButtons {
            pagination: false,
            render: false,
            miss_analyzer: false,
        },
    }
}
