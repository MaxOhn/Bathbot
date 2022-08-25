use std::sync::Arc;

use twilight_model::channel::embed::EmbedField;

use crate::{
    util::{
        builder::{EmbedBuilder, FooterBuilder, MessageBuilder},
        interaction::InteractionCommand,
        InteractionCommandExt,
    },
    BotResult, Context,
};

pub async fn trackingstats(ctx: Arc<Context>, command: InteractionCommand) -> BotResult<()> {
    let stats = ctx.tracking().stats().await;
    let entry = stats.next_pop;

    let fields = vec![
        EmbedField {
            name: "Currently tracking".to_owned(),
            value: stats.tracking.to_string(),
            inline: true,
        },
        EmbedField {
            name: "Interval per user".to_owned(),
            value: format!("{}s", stats.interval),
            inline: true,
        },
        EmbedField {
            name: "Min interval".to_owned(),
            value: format!("{}s", stats.wait_interval),
            inline: true,
        },
        EmbedField {
            name: "Milliseconds per user".to_owned(),
            value: format!("{}ms", stats.ms_per_track),
            inline: true,
        },
        EmbedField {
            name: "Next pop".to_owned(),
            value: format!("{} | {}", entry.user_id, entry.mode),
            inline: true,
        },
    ];

    let title = format!("Tracked users: {} | queue: {}", stats.users, stats.queue);

    let embed = EmbedBuilder::new()
        .footer(FooterBuilder::new("Last pop"))
        .timestamp(stats.last_pop)
        .title(title)
        .fields(fields)
        .build();

    let builder = MessageBuilder::new().embed(embed);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
