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
    },
    BotResult, Context,
};

pub(super) async fn medal_count(
    ctx: Arc<Context>,
    command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let owner = command.user_id()?;
    let osu_fut = ctx.psql().get_user_osu(owner);
    let osekai_fut = ctx.client().get_osekai_ranking::<MedalCount>();

    let (ranking, author_name) = match tokio::join!(osekai_fut, osu_fut) {
        (Ok(ranking), Ok(osu)) => (ranking, osu.map(OsuData::into_username)),
        (Ok(ranking), Err(err)) => {
            let report = Report::new(err).wrap_err("failed to retrieve user config");
            warn!("{:?}", report);

            (ranking, None)
        }
        (Err(err), _) => {
            let _ = command.error(&ctx, OSEKAI_ISSUE).await;

            return Err(err.into());
        }
    };

    let author_idx = author_name
        .as_deref()
        .and_then(|name| ranking.iter().position(|e| e.username.as_str() == name));

    let pages = numbers::div_euclid(10, ranking.len());
    let embed_data = MedalCountEmbed::new(&ranking[..10], 0, author_idx, (1, pages));
    let embed = embed_data.build();
    let builder = MessageBuilder::new().embed(embed);
    let response = command.update(&ctx, &builder).await?.model().await?;

    MedalCountPagination::new(response, ranking, author_idx).start(ctx, owner, 60);

    Ok(())
}
