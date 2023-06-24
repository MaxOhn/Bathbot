use std::sync::Arc;

use bathbot_util::{EmbedBuilder, FooterBuilder, MessageBuilder};
use eyre::Result;
use twilight_model::channel::message::embed::EmbedField;

use crate::{
    util::{interaction::InteractionCommand, InteractionCommandExt},
    Context,
};

pub async fn trackingstats(ctx: Arc<Context>, command: InteractionCommand) -> Result<()> {
    let stats = ctx.tracking().stats().await;

    let mut fields = vec![
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
    ];

    if let Some(entry) = stats.next_pop {
        fields.push(EmbedField {
            name: "Next pop".to_owned(),
            value: format!("{} | {}", entry.user_id, entry.mode),
            inline: true,
        });
    }

    let title = format!("Tracked users: {} | queue: {}", stats.users, stats.queue);

    let embed = EmbedBuilder::new()
        .footer(FooterBuilder::new("Last pop"))
        .timestamp(stats.last_pop)
        .title(title)
        .fields(fields);

    let builder = MessageBuilder::new().embed(embed);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
