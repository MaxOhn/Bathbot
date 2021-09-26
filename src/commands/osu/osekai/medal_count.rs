use crate::{
    custom_client::MedalCount,
    embeds::{EmbedData, MedalCountEmbed},
    util::constants::OSEKAI_ISSUE,
    util::{ApplicationCommandExt, MessageExt},
    BotResult, Context,
};

use std::sync::Arc;
use twilight_model::application::interaction::ApplicationCommand;

pub(super) async fn medal_count(ctx: Arc<Context>, command: ApplicationCommand) -> BotResult<()> {
    let config_fut = async {
        let user_id = command.user_id()?;

        ctx.user_config(user_id).await
    };

    let osekai_fut = ctx.clients.custom.get_osekai_ranking(MedalCount);

    let (ranking, author_name) = match tokio::join!(osekai_fut, config_fut) {
        (Ok(ranking), Ok(config)) => (ranking, config.osu_username),
        (Ok(ranking), Err(why)) => {
            unwind_error!(warn, why, "Failed to retrieve user config: {}");

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

    let embed_data = MedalCountEmbed::new(&ranking[..10], 0, author_idx);
    let builder = embed_data.into_builder().build().into();
    let response = command.create_message(&ctx, builder).await?;

    // TODO: Pagination

    Ok(())
}
