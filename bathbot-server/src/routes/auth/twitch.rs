use std::sync::Arc;

use axum::{
    extract::{rejection::QueryRejection, Query, State},
    http::StatusCode,
    response::Html,
};
use bathbot_model::{TwitchDataList, TwitchOAuthToken, TwitchUser};
use bathbot_util::constants::{TWITCH_OAUTH, TWITCH_USERS_ENDPOINT};
use eyre::Report;
use hyper::{header::AUTHORIZATION, Body, Request};

use super::{AuthError, Params, RenderData, RenderDataKind, RenderDataStatus};
use crate::state::AppState;

pub async fn auth_twitch(
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
        kind: RenderDataKind::Twitch,
    };

    match state.handlebars.render("auth", &render_data) {
        Ok(page) => Ok((status_code, Html(page))),
        Err(err) => {
            let wrap = "Failed to render error page";
            error!("{:?}", Report::new(err).wrap_err(wrap));

            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn auth(
    query: Result<Query<Params>, QueryRejection>,
    state: &AppState,
) -> Result<String, AuthError> {
    let Query(params) = query?;

    if state.standby.is_twitch_empty() {
        return Err(AuthError::EmptyStandby);
    }

    let req_uri = format!(
        "{TWITCH_OAUTH}?client_id={client_id}&client_secret={token}\
        &code={code}&grant_type=authorization_code&redirect_uri={redirect_base}/auth/twitch",
        client_id = state.twitch_client_id,
        token = state.twitch_token,
        code = params.code,
        redirect_base = state.redirect_base,
    );

    let token_req = Request::post(req_uri).body(Body::empty())?;

    let response = state
        .client
        .request(token_req)
        .await
        .map_err(AuthError::TwitchResponse)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(AuthError::ResponseBytes)?;

    let token: TwitchOAuthToken =
        serde_json::from_slice(&bytes).map_err(AuthError::DeserializeTwitch)?;

    let req_builder = Request::get(TWITCH_USERS_ENDPOINT)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("Client-ID", &state.twitch_client_id);

    let user_req = req_builder.body(Body::empty())?;

    let response = state
        .client
        .request(user_req)
        .await
        .map_err(AuthError::TwitchResponse)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(AuthError::ResponseBytes)?;

    let user = serde_json::from_slice::<TwitchDataList<TwitchUser>>(&bytes)
        .map_err(AuthError::DeserializeTwitch)?
        .data
        .pop()
        .ok_or(AuthError::EmptyTwitchData)?;

    let render_data = RenderData {
        status: RenderDataStatus::Success {
            name: &user.display_name,
        },
        kind: RenderDataKind::Twitch,
    };

    let page = state.handlebars.render("auth", &render_data)?;

    info!(
        "Successful twitch authorization for `{}`",
        user.display_name
    );

    state.standby.process_twitch(user, params.state);

    Ok(page)
}
