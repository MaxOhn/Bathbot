use crate::{
    embeds::{EmbedData, RankingCountriesEmbed},
    pagination::{Pagination, RankingCountriesPagination},
    util::{constants::OSU_API_ISSUE, numbers, MessageExt},
    Args, BotResult, Context,
};

use rosu_v2::prelude::GameMode;
use std::{collections::BTreeMap, sync::Arc};
use twilight_model::channel::Message;

async fn country_ranking_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    _: Args<'_>,
) -> BotResult<()> {
    let mut ranking = match ctx.osu().country_rankings(mode).await {
        Ok(ranking) => ranking,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let pages = numbers::div_euclid(15, ranking.total as usize);
    let countries: BTreeMap<_, _> = ranking.ranking.drain(..).enumerate().collect();
    let data = RankingCountriesEmbed::new(mode, &countries, (1, pages));

    // Creating the embed
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embeds(&[data.into_builder().build()])?.exec().await?.model()
        .await?;

    // Pagination
    let pagination = RankingCountriesPagination::new(
        response,
        mode,
        Arc::clone(&ctx),
        ranking.total as usize,
        countries,
    );

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (countryranking): {}")
        }
    });

    Ok(())
}

#[command]
#[short_desc("Display the osu! rankings for countries")]
#[aliases("cr")]
pub async fn countryranking(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    country_ranking_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display the osu!mania rankings for countries")]
#[aliases("crm")]
pub async fn countryrankingmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    country_ranking_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display the osu!taiko rankings for countries")]
#[aliases("crt")]
pub async fn countryrankingtaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    country_ranking_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display the osu!ctb rankings for countries")]
#[aliases("crc")]
pub async fn countryrankingctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    country_ranking_main(GameMode::CTB, ctx, msg, args).await
}
