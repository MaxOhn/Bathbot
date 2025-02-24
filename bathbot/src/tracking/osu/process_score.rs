use std::{slice, sync::Arc, time::Duration};

use bathbot_model::embed_builder::{
    ComboValue, HitresultsValue, ScoreEmbedSettings, SettingValue, SettingsButtons, SettingsImage,
    Value,
};
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::{EmbedBuilder, constants::UNKNOWN_CHANNEL};
use rand::Rng;
use rosu_v2::{model::GameMode, prelude::Score};
use twilight_http::{
    api_error::{ApiError, GeneralApiError},
    error::ErrorType as TwilightErrorType,
};
use twilight_model::id::Id;

use super::{OsuTracking, entry::TrackEntry};
use crate::{
    active::impls::{MarkIndex, SingleScoreContent, SingleScorePagination},
    commands::utility::ScoreEmbedDataWrap,
    core::{BotMetrics, Context},
    manager::{
        OsuMap,
        redis::osu::{CachedUser, UserArgs, UserArgsSlim},
    },
};

pub async fn process_score(score: Score, entry: Arc<TrackEntry>) {
    let Some(pp) = score.pp else { return };

    // Add delay to improve chances that the score was processed fully and will
    // appear in the user's top100 scores. The jitter in the delay should
    // improve db & api congestion.
    tokio::time::sleep(jitter()).await;

    let user_id = score.user_id;
    let score_id = score.id;
    let map_id = score.map_id;
    let mode = score.mode;

    let user_args = UserArgsSlim::user_id(user_id).mode(mode);
    let user_fut = Context::redis().osu_user(UserArgs::Args(user_args));
    let tops_fut = Context::osu_scores().top(false).limit(100).exec(user_args);

    let checksum = score.map.as_ref().and_then(|map| map.checksum.as_deref());
    let map_fut = Context::osu_map().map(map_id, checksum);

    let (user, tops, map) = match tokio::join!(user_fut, tops_fut, map_fut) {
        (Ok(user), Ok(scores), Ok(map)) => (user, scores, map),
        (Err(err), ..) => {
            log!(warn: user = user_id, ?mode, score_id, ?err, "Failed to get user");

            return;
        }
        (_, Err(err), _) => {
            log!(warn:
                user = user_id,
                ?mode,
                score_id,
                ?err,
                "Failed to get top scores"
            );

            return;
        }
        (.., Err(err)) => {
            log!(warn:
                map = map_id,
                user = user_id,
                score_id,
                ?err,
                "Failed to get map"
            );

            return;
        }
    };

    entry.insert_last_pp(user_id, mode, &tops).await;

    let Some(idx) = tops.iter().position(|s| s.id == score_id) else {
        log!(info:
            user = user_id,
            map = map_id,
            score_id,
            pp,
            "Not in top scores",
        );

        return;
    };

    BotMetrics::osu_tracking_hit(score.mode);

    let combo = score.max_combo;
    let (builder, max_combo) = embed_builder(&user, score, map, idx).await;
    let idx = idx as u8 + 1;
    let embed = builder.build();
    let embeds = slice::from_ref(&embed);
    let combo_percent = max_combo.map(|max| 100.0 * combo as f32 / max as f32);

    log!(info:
        user = user_id,
        map = map_id,
        score_id,
        idx,
        pp,
        combo_percent,
        "New top score",
    );

    let http = Context::http();

    let channels: Vec<_> = entry
        .channels()
        .iter()
        .filter_map(|(channel_id, params)| {
            params
                .matches(idx, pp, combo_percent)
                .then_some(*channel_id)
        })
        .collect();

    for channel_id in channels {
        let channel = Id::new(channel_id.get());

        let Err(err) = http.create_message(channel).embeds(embeds).await else {
            continue;
        };

        let TwilightErrorType::Response { error, .. } = err.kind() else {
            log!(warn: %channel, ?err, "Error while sending notif");

            continue;
        };

        let ApiError::General(GeneralApiError {
            code: UNKNOWN_CHANNEL,
            ..
        }) = error
        else {
            log!(warn: %channel, ?error, "Error from API while sending notif");

            continue;
        };

        OsuTracking::remove_channel(channel, None).await;
    }
}

/// Random [`Duration`] between 30s and 60s
fn jitter() -> Duration {
    rand::thread_rng().gen_range(Duration::from_secs(30)..Duration::from_secs(60))
}

async fn embed_builder(
    user: &CachedUser,
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
