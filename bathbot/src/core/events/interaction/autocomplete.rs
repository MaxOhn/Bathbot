use std::{mem, sync::Arc, time::Instant};

use crate::{
    commands::{
        help::slash_help,
        osu::{slash_badges, slash_cs, slash_medal, slash_regiontop},
    },
    core::{events::EventKind, BotMetrics, Context},
    util::interaction::InteractionCommand,
};

pub async fn handle_autocomplete(ctx: Arc<Context>, mut command: InteractionCommand) {
    let start = Instant::now();

    let name = mem::take(&mut command.data.name);
    EventKind::Autocomplete.log(&ctx, &command, &name).await;

    let res = match name.as_str() {
        "help" => slash_help(ctx, command).await,
        "badges" => slash_badges(ctx, command).await,
        "medal" => slash_medal(ctx, command).await,
        "cs" | "compare" | "score" => slash_cs(ctx, command).await,
        "regiontop" => slash_regiontop(ctx, command).await,
        _ => return error!(name, "Unknown autocomplete command"),
    };

    if let Err(err) = res {
        BotMetrics::inc_command_error("autocomplete", name.clone());
        error!(name, ?err, "Failed to process autocomplete");
    }

    let elapsed = start.elapsed();
    BotMetrics::observe_command("autocomplete", name, elapsed);
}
