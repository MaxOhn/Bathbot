use bathbot_util::{MessageBuilder, constants::GENERAL_ISSUE};
use eyre::{Report, Result};
use twilight_model::id::Id;

use crate::{
    core::Context,
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

pub async fn request_members(command: InteractionCommand, guild_id: &str) -> Result<()> {
    let Ok(Some(guild)) = guild_id.parse().map(Id::new_checked) else {
        command
            .error_callback("Must provide a valid guild id")
            .await?;

        return Ok(());
    };

    let ctx = Context::get();

    let Some(shard_id) = ctx.guild_shards().pin().get(&guild).copied() else {
        let content = format!("No stored shard id for guild {guild}");
        command.error_callback(content).await?;

        return Ok(());
    };

    ctx.member_requests
        .pending_guilds
        .lock()
        .unwrap()
        .insert(guild);

    match ctx.member_requests.tx.send((guild, shard_id)) {
        Ok(_) => {
            let content = "Successfully enqueued member request";
            let builder = MessageBuilder::new().embed(content);
            command.callback(builder, false).await?;

            Ok(())
        }
        Err(err) => {
            let _ = command.error_callback(GENERAL_ISSUE).await;

            Err(Report::new(err).wrap_err("Failed to forward member request"))
        }
    }
}
