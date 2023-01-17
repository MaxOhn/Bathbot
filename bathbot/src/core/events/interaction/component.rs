use std::{mem, sync::Arc};

use crate::{
    commands::help::{handle_help_category, handle_help_component},
    core::{events::EventKind, Context},
    games::{bg::components::*, hl::components::*},
    pagination::components::*,
    util::interaction::InteractionComponent,
};

pub async fn handle_component(ctx: Arc<Context>, mut component: InteractionComponent) {
    let name = mem::take(&mut component.data.custom_id);
    EventKind::Component.log(&ctx, &component, &name);
    ctx.stats.increment_component(&name);

    let res = match name.as_str() {
        "help_menu" | "help_back" => handle_help_component(&ctx, component).await,
        "bg_start_include" => handle_bg_start_include(&ctx, component).await,
        "bg_start_exclude" => handle_bg_start_exclude(&ctx, component).await,
        "bg_start_effects" => handle_bg_start_effects(&ctx, component).await,
        "bg_start_button" => handle_bg_start_button(ctx, component).await,
        "bg_start_cancel" => handle_bg_start_cancel(&ctx, component).await,
        "help_category" => handle_help_category(&ctx, component).await,
        "higher_button" => handle_higher(ctx, component).await,
        "lower_button" => handle_lower(ctx, component).await,
        "try_again_button" => handle_try_again(ctx, component).await,
        "next_higherlower" => handle_next_higherlower(ctx, component).await,
        "pagination_start" => handle_pagination_start(ctx, component).await,
        "pagination_back" => handle_pagination_back(ctx, component).await,
        "pagination_custom" => handle_pagination_custom(ctx, component).await,
        "pagination_step" => handle_pagination_step(ctx, component).await,
        "pagination_end" => handle_pagination_end(ctx, component).await,
        "profile_menu" => handle_profile_menu(ctx, component).await,
        "sim_mods" => handle_sim_mods_button(ctx, component).await,
        "sim_combo" => handle_sim_combo_button(ctx, component).await,
        "sim_acc" => handle_sim_acc_button(ctx, component).await,
        "sim_clock_rate" => handle_sim_clock_rate_button(ctx, component).await,
        "sim_geki" => handle_sim_geki_button(ctx, component).await,
        "sim_katu" => handle_sim_katu_button(ctx, component).await,
        "sim_n300" => handle_sim_n300_button(ctx, component).await,
        "sim_n100" => handle_sim_n100_button(ctx, component).await,
        "sim_n50" => handle_sim_n50_button(ctx, component).await,
        "sim_miss" => handle_sim_miss_button(ctx, component).await,
        "sim_score" => handle_sim_score_button(ctx, component).await,
        "sim_osu_version" | "sim_taiko_version" | "sim_catch_version" | "sim_mania_version" => {
            handle_sim_version(ctx, component).await
        }
        "sim_attrs" => handle_sim_attrs_button(ctx, component).await,
        _ => return error!("Unknown message component `{name}`"),
    };

    if let Err(err) = res {
        let wrap = format!("Failed to process component `{name}`");
        error!("{:?}", err.wrap_err(wrap));
    }
}
