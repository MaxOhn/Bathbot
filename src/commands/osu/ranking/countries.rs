use crate::{
    commands::GameModeOption,
    core::commands::CommandOrigin,
    embeds::{EmbedData, RankingCountriesEmbed},
    pagination::{Pagination, RankingCountriesPagination},
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers,
    },
    BotResult, Context,
};

use command_macros::command;
use rosu_v2::prelude::GameMode;
use std::{collections::BTreeMap, sync::Arc};

use super::RankingCountry;

#[command]
#[desc("Display the osu! rankings for countries")]
#[aliases("cr")]
#[group(Osu)]
pub async fn prefix_countryranking(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    country(ctx, msg.into(), None.into()).await
}

#[command]
#[desc("Display the osu!mania rankings for countries")]
#[aliases("crm")]
#[group(Mania)]
pub async fn prefix_countryrankingmania(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    country(ctx, msg.into(), Some(GameModeOption::Mania).into()).await
}

#[command]
#[desc("Display the osu!taiko rankings for countries")]
#[aliases("crt")]
#[group(Taiko)]
pub async fn prefix_countryrankingtaiko(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    country(ctx, msg.into(), Some(GameModeOption::Taiko).into()).await
}

#[command]
#[desc("Display the osu!ctb rankings for countries")]
#[aliases("crc", "countryrankingcatch")]
#[group(Catch)]
pub async fn prefix_countryrankingctb(ctx: Arc<Context>, msg: &Message) -> BotResult<()> {
    country(ctx, msg.into(), Some(GameModeOption::Catch).into()).await
}

pub(super) async fn country(
    ctx: Arc<Context>,
    orig: CommandOrigin<'_>,
    args: RankingCountry,
) -> BotResult<()> {
    let owner = orig.user_id()?;

    let mode = match args.mode {
        Some(mode) => mode.into(),
        None => match ctx.user_config(owner).await {
            Ok(config) => config.mode.unwrap_or(GameMode::STD),
            Err(err) => {
                let _ = orig.error(&ctx, GENERAL_ISSUE).await;

                return Err(err);
            }
        },
    };

    let mut ranking = match ctx.osu().country_rankings(mode).await {
        Ok(ranking) => ranking,
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    // Creating the embed
    let pages = numbers::div_euclid(15, ranking.total as usize);
    let countries: BTreeMap<_, _> = ranking.ranking.drain(..).enumerate().collect();
    let embed_data = RankingCountriesEmbed::new(mode, &countries, (1, pages));
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    let response = orig.create_message(&ctx, &builder).await?.model().await?;

    // Pagination
    let pagination = RankingCountriesPagination::new(
        response,
        mode,
        Arc::clone(&ctx),
        ranking.total as usize,
        countries,
    );

    pagination.start(ctx, owner, 60);

    Ok(())
}
