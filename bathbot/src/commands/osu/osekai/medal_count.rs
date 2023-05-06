use std::{borrow::Cow, sync::Arc};

use bathbot_model::{Countries, MedalCount};
use bathbot_util::constants::OSEKAI_ISSUE;
use eyre::Result;

use super::OsekaiMedalCount;
use crate::{
    active::{impls::MedalCountPagination, ActiveMessages},
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
    Context,
};

pub(super) async fn medal_count(
    ctx: Arc<Context>,
    mut command: InteractionCommand,
    args: OsekaiMedalCount,
) -> Result<()> {
    let country_code = match args.country {
        Some(country) => {
            if country.len() == 2 {
                Some(Cow::Owned(country))
            } else if let Some(code) = Countries::name(&country).to_code() {
                Some(code.into())
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
    let ranking_fut = ctx.redis().osekai_ranking::<MedalCount>();
    let config_fut = ctx.user_config().osu_name(owner);

    let (osekai_res, name_res) = tokio::join!(ranking_fut, config_fut);

    let mut ranking = match osekai_res {
        Ok(ranking) => ranking.into_original(),
        Err(err) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.wrap_err("failed to get cached medal count ranking"));
        }
    };

    let author_name = match name_res {
        Ok(name_opt) => name_opt,
        Err(err) => {
            warn!(?err, "Failed to get username");

            None
        }
    };

    if let Some(code) = country_code {
        let code = code.to_ascii_uppercase();

        ranking.retain(|entry| entry.country_code == code);
    }

    let author_idx = author_name
        .as_deref()
        .and_then(|name| ranking.iter().position(|e| e.username.as_str() == name));

    let pagination = MedalCountPagination::builder()
        .ranking(ranking.into_boxed_slice())
        .author_idx(author_idx)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}
