use std::sync::Arc;

use axum::extract::{rejection::QueryRejection, Query, State};
use rosu_v2::Osu;

use crate::{error::AppError, state::AppState};

use super::{Params, RenderData, RenderDataKind, RenderDataStatus};

pub async fn auth_osu(
    query: Result<Query<Params>, QueryRejection>,
    State(state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let Query(params) = query?;

    if state.standby.is_osu_empty() {
        return Err(AppError::EmptyStandby);
    }

    let osu = Osu::builder()
        .client_id(state.osu_client_id)
        .client_secret(&state.osu_client_secret)
        .with_authorization(params.code, &state.osu_redirect)
        .build()
        .await
        .map_err(AppError::OsuAuthClient)?;

    let user = osu.own_data().await.map_err(AppError::OsuApi)?;

    let render_data = RenderData {
        status: RenderDataStatus::Success,
        kind: RenderDataKind::Osu,
        name: &user.username,
    };

    let page = state.handlebars.render("auth", &render_data)?;
    info!("Successful osu! authorization for `{}`", user.username);
    state.standby.process_osu(user, params.state);

    Ok(page)
}
