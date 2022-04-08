use std::{mem, sync::Arc};

use eyre::Report;
use twilight_model::application::interaction::ApplicationCommandAutocomplete;

use crate::{
    commands::help::handle_help_autocomplete,
    core::{events::log_command, Context},
};

pub async fn handle_autocomplete(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommandAutocomplete>,
) {
    let name = mem::take(&mut command.data.name);
    log_command(&ctx, &command, &name);
    ctx.stats.increment_autocomplete(&name);

    let res = match name.as_str() {
        "help" => handle_help_autocomplete(ctx, command).await,
        // TODO: "badges" & "medal"
        _ => return error!("unknown autocomplete command `{name}`"),
    };

    if let Err(err) = res {
        let wrap = format!("failed to process autocomplete `{name}`");
        error!("{:?}", Report::new(err).wrap_err(wrap));
    }
}
