use std::sync::Arc;

use axum::{
    extract::{rejection::QueryRejection, Query, State},
    http::StatusCode,
    response::Html,
};
use eyre::Report;
use rosu_v2::Osu;

use super::{AuthError, Params, RenderData, RenderDataKind, RenderDataStatus};
use crate::state::AppState;

pub async fn auth_osu(
    query: Result<Query<Params>, QueryRejection>,
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Html<String>), StatusCode> {
    let err = match auth(query, &state).await {
        Ok(page) => return Ok((StatusCode::OK, Html(page))),
        Err(err) => err,
    };

    let (status_code, msg) = err.response();
    warn!("{:?}", Report::new(err));

    let render_data = RenderData {
        status: RenderDataStatus::Error { msg },
        kind: RenderDataKind::Osu,
    };

    match state.handlebars.render("auth", &render_data) {
        Ok(page) => Ok((status_code, Html(page))),
        Err(err) => {
            error!(?err, "Failed to render error page");

            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn auth(
    query: Result<Query<Params>, QueryRejection>,
    state: &AppState,
) -> Result<String, AuthError> {
    let Query(params) = query?;

    if state.standby.is_osu_empty() {
        return Err(AuthError::EmptyStandby);
    }

    let mut redirect = state.redirect_base.to_string();
    redirect.push_str("/auth/osu");

    let osu = Osu::builder()
        .client_id(state.osu_client_id)
        .client_secret(&*state.osu_client_secret)
        .with_authorization(params.code, &*redirect)
        .build()
        .await
        .map_err(AuthError::OsuAuthClient)?;

    let user = osu.own_data().await.map_err(AuthError::OsuApi)?;

    let render_data = RenderData {
        status: RenderDataStatus::Success {
            name: &user.username,
        },
        kind: RenderDataKind::Osu,
    };

    let page = state.handlebars.render("auth", &render_data)?;

    info!(
        name = user.username.as_str(),
        "Successful osu! authorization"
    );

    state.standby.process_osu(user, params.state);

    Ok(page)
}
