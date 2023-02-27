use std::{mem, sync::Arc};

use crate::{
    commands::{
        help::slash_help,
        osu::{slash_badges, slash_cs, slash_medal},
    },
    core::{events::EventKind, Context},
    util::interaction::InteractionCommand,
};

pub async fn handle_autocomplete(ctx: Arc<Context>, mut command: InteractionCommand) {
    let name = mem::take(&mut command.data.name);
    EventKind::Autocomplete.log(&ctx, &command, &name).await;

    let res = match name.as_str() {
        "help" => slash_help(ctx, command).await,
        "badges" => slash_badges(ctx, command).await,
        "medal" => slash_medal(ctx, command).await,
        "cs" | "compare" | "score" => slash_cs(ctx, command).await,
        _ => return error!("Unknown autocomplete command `{name}`"),
    };

    if let Err(err) = res {
        let wrap = format!("Failed to process autocomplete `{name}`");
        error!("{:?}", err.wrap_err(wrap));
    }
}
