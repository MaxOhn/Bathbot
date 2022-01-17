use crate::{
    custom_client::MedalCount,
    database::OsuData,
    embeds::{EmbedData, MedalCountEmbed},
    pagination::{MedalCountPagination, Pagination},
    util::{constants::OSEKAI_ISSUE, numbers, InteractionExt, MessageExt},
    BotResult, Context,
};

use eyre::Report;
use std::sync::Arc;
use twilight_model::application::interaction::ApplicationCommand;

pub(super) async fn medal_count(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let owner = command.user_id()?;
    let osu_fut = ctx.psql().get_user_osu(owner);
    let osekai_fut = ctx.clients.custom.get_osekai_ranking::<MedalCount>();

    let (ranking, author_name) = match tokio::join!(osekai_fut, osu_fut) {
        (Ok(ranking), Ok(osu)) => (ranking, osu.map(OsuData::into_username)),
        (Ok(ranking), Err(why)) => {
            let report = Report::new(why).wrap_err("failed to retrieve user config");
            warn!("{:?}", report);

            (ranking, None)
        }
        (Err(why), _) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(why.into());
        }
    };

    let author_idx = author_name.as_deref().and_then(|name| {
        ranking
            .iter()
            .position(|entry| entry.username.as_str() == name)
    });

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalCountEmbed::new(&ranking[..10], 0, author_idx, (1, pages));
    let builder = embed_data.into_builder().build().into();
    let response = command.create_message(&ctx, builder).await?.model().await?;
    let pagination = MedalCountPagination::new(response, ranking, author_idx);

    tokio::spawn(async move {
        if let Err(err) = pagination.start(&ctx, owner, 60).await {
            warn!("{:?}", Report::new(err));
        }
    });

    Ok(())
}
