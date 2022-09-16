use std::{mem, sync::Arc};

use crate::{
    core::{events::log_command, Context},
    pagination::components::handle_pagination_modal,
    util::interaction::InteractionModal,
};

pub async fn handle_modal(ctx: Arc<Context>, mut modal: InteractionModal) {
    let name = mem::take(&mut modal.data.custom_id);
    log_command(&ctx, &modal, &name);
    ctx.stats.increment_modal(&name);

    let res = match name.as_str() {
        "pagination_page" => handle_pagination_modal(ctx, modal).await,
        _ => return error!("Unknown modal `{name}`: {modal:#?}"),
    };

    if let Err(err) = res {
        let wrap = format!("Failed to process modal `{name}`");
        error!("{:?}", err.wrap_err(wrap));
    }
}
