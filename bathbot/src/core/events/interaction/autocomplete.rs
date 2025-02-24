use std::{mem, time::Instant};

use crate::{
    commands::{
        help::slash_help,
        osu::{slash_badges, slash_cs, slash_medal},
    },
    core::{BotMetrics, events::EventKind},
    util::interaction::InteractionCommand,
};

pub async fn handle_autocomplete(mut command: InteractionCommand) {
    let start = Instant::now();

    let name = mem::take(&mut command.data.name);
    EventKind::Autocomplete.log(&command, &name).await;

    let res = match name.as_str() {
        "help" => slash_help(command).await,
        "badges" => slash_badges(command).await,
        "medal" => slash_medal(command).await,
        "cs" | "compare" | "score" => slash_cs(command).await,
        _ => return error!(name, "Unknown autocomplete command"),
    };

    if let Err(err) = res {
        BotMetrics::inc_command_error("autocomplete", name.clone());
        error!(name, ?err, "Failed to process autocomplete");
    }

    let elapsed = start.elapsed();
    BotMetrics::observe_command("autocomplete", name, elapsed);
}
