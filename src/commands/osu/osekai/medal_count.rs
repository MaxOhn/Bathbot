use std::sync::Arc;

use eyre::Report;
use twilight_model::application::interaction::ApplicationCommand;

use crate::{
    custom_client::MedalCount,
    database::OsuData,
    embeds::{EmbedData, MedalCountEmbed},
    pagination::{MedalCountPagination, Pagination},
    util::{
        builder::MessageBuilder, constants::OSEKAI_ISSUE, numbers, ApplicationCommandExt, Authored,
        CountryCode,
    },
    BotResult, Context,
};

use super::OsekaiMedalCount;

pub(super) async fn medal_count(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
    args: OsekaiMedalCount,
) -> BotResult<()> {
    let country_code = match args.country {
        Some(country) => {
            if country.len() == 2 {
                Some(country.into())
            } else if let Some(code) = CountryCode::from_name(&country) {
                Some(code)
            } else {
                let content =
                    format!("Looks like `{country}` is neither a country name nor a country code");

                command.error(&ctx, content).await?;

                return Ok(());
            }
        }
        None => None,
    };

    let owner = command.user_id()?;
    let osu_fut = ctx.psql().get_user_osu(owner);
    let redis = ctx.redis();
    let osekai_fut = redis.osekai_ranking::<MedalCount>();

    let (mut ranking, author_name) = match tokio::join!(osekai_fut, osu_fut) {
        (Ok(ranking), Ok(osu)) => (ranking.to_inner(), osu.map(OsuData::into_username)),
        (Ok(ranking), Err(err)) => {
            let report = Report::new(err).wrap_err("failed to retrieve user config");
            warn!("{:?}", report);

            (ranking.to_inner(), None)
        }
        (Err(err), _) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();

        ranking.retain(|entry| entry.country_code == code);
    }

    let author_idx = author_name
        .as_deref()
        .and_then(|name| ranking.iter().position(|e| e.username.as_str() == name));

    let initial_ranking = &ranking[..ranking.len().min(10)];

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalCountEmbed::new(initial_ranking, 0, author_idx, (1, pages));
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    let response_raw = command.update(&ctx, &builder).await?;

    if ranking.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    MedalCountPagination::new(response, ranking, author_idx).start(ctx, owner, 60);

    Ok(())
}
