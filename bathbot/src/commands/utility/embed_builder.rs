use bathbot_macros::SlashCommand;
use bathbot_model::ScoreSlim;
use bathbot_psql::model::configs::ScoreData;
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::Score};
use twilight_interactions::command::CreateCommand;

use crate::{
    active::{
        impls::{
            ScoreEmbedBuilderActive, ScoreEmbedBuilderImage, ScoreEmbedBuilderPp,
            ScoreEmbedBuilderSettings, ScoreEmbedBuilderTimestamp,
        },
        ActiveMessages,
    },
    commands::ShowHideOption,
    core::Context,
    manager::{redis::osu::UserArgsSlim, OsuMap},
    util::{interaction::InteractionCommand, osu::IfFc, Authored, InteractionCommandExt},
};

const USER_ID: u32 = 2;
const MAP_ID: u32 = 197337;
const MAP_CHECKSUM: &'static str = "a708a5b90349e98b399f2a1c9fce5422";

#[derive(CreateCommand, SlashCommand)]
#[command(name = "builder", desc = "Build your own score embed format")]
pub struct ScoreEmbedBuilder;

pub async fn slash_scoreembedbuilder(mut command: InteractionCommand) -> Result<()> {
    let legacy_scores = false; // TODO
    let score_data = ScoreData::LazerWithClassicScoring;

    let msg_owner = command.user_id()?;

    let user_fut = Context::redis().osu_user_from_args(UserArgsSlim::user_id(USER_ID));

    let score_fut = Context::osu_scores().user_on_map_single(
        USER_ID,
        MAP_ID,
        GameMode::Osu,
        None,
        legacy_scores,
    );

    let map_fut = Context::osu_map().map(MAP_ID, Some(MAP_CHECKSUM));

    let (user, score, map) = match tokio::join!(user_fut, score_fut, map_fut) {
        (Ok(user), Ok(score), Ok(map)) => (user, score.score, map),
        (user_res, score_res, map_res) => {
            let _ = command.error(GENERAL_ISSUE).await;

            let (err, wrap) = if let Err(err) = user_res {
                (Report::new(err), "Failed to get user for builder")
            } else if let Err(err) = score_res {
                (Report::new(err), "Failed to get score for builder")
            } else if let Err(err) = map_res {
                (Report::new(err), "Failed to get map for builder")
            } else {
                unreachable!()
            };

            return Err(err.wrap_err(wrap));
        }
    };

    let data = ScoreEmbedBuilderData::new(score, map).await;

    let settings = ScoreEmbedBuilderSettings {
        image: ScoreEmbedBuilderImage::Thumbnail,
        pp: ScoreEmbedBuilderPp::Max,
        map_info: ShowHideOption::Show,
        footer: ShowHideOption::Show,
        timestamp: ScoreEmbedBuilderTimestamp::ScoreDate,
    };

    let pagination = ScoreEmbedBuilderActive::new(user, data, settings, score_data, msg_owner);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}

pub struct ScoreEmbedBuilderData {
    pub score: ScoreSlim,
    pub map: OsuMap,
    pub if_fc: Option<IfFc>,
    pub max_pp: f32,
    pub stars: f32,
    pub max_combo: u32,
}

impl ScoreEmbedBuilderData {
    async fn new(score: Score, map: OsuMap) -> Self {
        let mut calc = Context::pp(&map).mode(score.mode).mods(&score.mods);
        let attrs = calc.performance().await;

        let max_pp = attrs.pp() as f32;

        let pp = match score.pp {
            Some(pp) => pp,
            None => calc.score(&score).performance().await.pp() as f32,
        };

        let score = ScoreSlim::new(score, pp);
        let if_fc = IfFc::new(&score, &map).await;

        Self {
            score,
            map,
            if_fc,
            stars: attrs.stars() as f32,
            max_pp,
            max_combo: attrs.max_combo(),
        }
    }
}
