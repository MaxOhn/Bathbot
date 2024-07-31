use std::collections::BTreeMap;

use bathbot_macros::command;
use bathbot_model::command_fields::GameModeOption;
use bathbot_util::constants::{GENERAL_ISSUE, OSU_API_ISSUE};
use eyre::{Report, Result};
use rosu_v2::prelude::GameMode;

use super::RankingCountry;
use crate::{
    active::{impls::RankingCountriesPagination, ActiveMessages},
    core::commands::CommandOrigin,
    Context,
};

#[command]
#[desc("Display the osu! rankings for countries")]
#[aliases("cr")]
#[group(Osu)]
pub async fn prefix_countryranking(msg: &Message) -> Result<()> {
    country(msg.into(), None.into()).await
}

#[command]
#[desc("Display the osu!mania rankings for countries")]
#[aliases("crm")]
#[group(Mania)]
pub async fn prefix_countryrankingmania(msg: &Message) -> Result<()> {
    country(msg.into(), Some(GameModeOption::Mania).into()).await
}

#[command]
#[desc("Display the osu!taiko rankings for countries")]
#[aliases("crt")]
#[group(Taiko)]
pub async fn prefix_countryrankingtaiko(msg: &Message) -> Result<()> {
    country(msg.into(), Some(GameModeOption::Taiko).into()).await
}

#[command]
#[desc("Display the osu!ctb rankings for countries")]
#[aliases("crc", "countryrankingcatch")]
#[group(Catch)]
pub async fn prefix_countryrankingctb(msg: &Message) -> Result<()> {
    country(msg.into(), Some(GameModeOption::Catch).into()).await
}

pub(super) async fn country(orig: CommandOrigin<'_>, args: RankingCountry) -> Result<()> {
    let owner = orig.user_id()?;

    let mode = match args.mode {
        Some(mode) => mode.into(),
        None => match Context::user_config().mode(owner).await {
            Ok(mode) => mode.unwrap_or(GameMode::Osu),
            Err(err) => {
                let _ = orig.error(GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let mut ranking = match Context::osu().country_rankings(mode).await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = orig.error(OSU_API_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get country ranking");

            return Err(err);
        }
    };

    let countries: BTreeMap<_, _> = ranking.ranking.drain(..).enumerate().collect();

    let pagination = RankingCountriesPagination::builder()
        .mode(mode)
        .countries(countries)
        .total(ranking.total as usize)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}
