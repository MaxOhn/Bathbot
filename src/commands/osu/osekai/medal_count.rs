use std::sync::Arc;

use eyre::Result;

use crate::{
    custom_client::MedalCount,
    database::OsuData,
    pagination::MedalCountPagination,
    util::{
        constants::OSEKAI_ISSUE, interaction::InteractionCommand, Authored, CountryCode,
        InteractionCommandExt,
    },
    Context,
};

use super::OsekaiMedalCount;

pub(super) async fn medal_count(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: OsekaiMedalCount,
) -> Result<()> {
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
            warn!("{:?}", err.wrap_err("Failed to get username"));

            (ranking.to_inner(), None)
        }
        (Err(err), _) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medal count ranking"));
        }
    };

    if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();

        ranking.retain(|entry| entry.country_code == code);
    }

    let author_idx = author_name
        .as_deref()
        .and_then(|name| ranking.iter().position(|e| e.username.as_str() == name));

    MedalCountPagination::builder(ranking, author_idx)
        .start_by_update()
        .start(ctx, (&mut command).into())
        .await
}
