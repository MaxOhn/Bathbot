use std::{mem, sync::Arc};

use crate::{
    core::{events::EventKind, Context},
    pagination::components::*,
    util::interaction::InteractionModal,
};

pub async fn handle_modal(ctx: Arc<Context>, mut modal: InteractionModal) {
    let name = mem::take(&mut modal.data.custom_id);
    EventKind::Modal.log(&ctx, &modal, &name);
    ctx.stats.increment_modal(&name);

    let res = match name.as_str() {
        "pagination_page" => handle_pagination_modal(ctx, modal).await,
        "sim_mods" => handle_sim_mods_modal(ctx, modal).await,
        "sim_combo" => handle_sim_combo_modal(ctx, modal).await,
        "sim_acc" => handle_sim_acc_modal(ctx, modal).await,
        "sim_clock_rate" => handle_sim_clock_rate_modal(ctx, modal).await,
        "sim_geki" => handle_sim_geki_modal(ctx, modal).await,
        "sim_katu" => handle_sim_katu_modal(ctx, modal).await,
        "sim_n300" => handle_sim_n300_modal(ctx, modal).await,
        "sim_n100" => handle_sim_n100_modal(ctx, modal).await,
        "sim_n50" => handle_sim_n50_modal(ctx, modal).await,
        "sim_miss" => handle_sim_miss_modal(ctx, modal).await,
        "sim_score" => handle_sim_score_modal(ctx, modal).await,
        _ => return error!("Unknown modal `{name}`: {modal:#?}"),
    };

    if let Err(err) = res {
        let wrap = format!("Failed to process modal `{name}`");
        error!("{:?}", err.wrap_err(wrap));
    }
}
