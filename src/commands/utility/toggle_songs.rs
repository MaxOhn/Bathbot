use crate::{
    util::{constants::GENERAL_ISSUE, MessageExt},
    BotResult, CommandData, Context, MessageBuilder,
};

use std::sync::Arc;

#[command]
#[only_guilds()]
#[authority()]
#[short_desc("Toggle availability of song commands in a server")]
#[long_desc(
    "Toggle whether song commands can be used in this server. \
    Defaults to `true`"
)]
#[aliases("songstoggle", "songtoggle")]
async fn togglesongs(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    _togglesongs(ctx, data, None).await
}

async fn _togglesongs(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    value: Option<bool>,
) -> BotResult<()> {
    let guild_id = data.guild_id().unwrap();
    let mut with_lyrics = false;

    let update_fut = ctx.update_guild_config(guild_id, |config| {
        config.with_lyrics = if value.is_some() {
            value
        } else {
            Some(!config.with_lyrics())
        };

        with_lyrics = config.with_lyrics();
    });

    if let Err(why) = update_fut.await {
        let _ = data.error(&ctx, GENERAL_ISSUE).await;

        return Err(why);
    }

    let content = if with_lyrics {
        "Song commands can now be used in this server"
    } else {
        "Song commands can no longer be used in this server"
    };

    let builder = MessageBuilder::new().embed(content);
    data.create_message(&ctx, builder).await?;

    Ok(())
}
