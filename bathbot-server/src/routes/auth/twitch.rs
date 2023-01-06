use std::sync::Arc;

use axum::extract::{rejection::QueryRejection, Query, State};
use bathbot_model::{TwitchDataList, TwitchOAuthToken, TwitchUser};
use bathbot_util::constants::{TWITCH_OAUTH, TWITCH_USERS_ENDPOINT};
use hyper::{header::AUTHORIZATION, Body, Request};

use crate::{error::AppError, AppState};

use super::{Params, RenderData, RenderDataKind, RenderDataStatus};

pub async fn auth_twitch(
    query: Result<Query<Params>, QueryRejection>,
    State(state): State<Arc<AppState>>,
) -> Result<String, AppError> {
    let Query(params) = query?;

    if state.standby.is_twitch_empty() {
        return Err(AppError::EmptyStandby);
    }

    let req_uri = format!(
        "{TWITCH_OAUTH}?client_id={client_id}&client_secret={token}\
        &code={code}&grant_type=authorization_code&redirect_uri={redirect}",
        client_id = state.twitch_client_id,
        token = state.twitch_token,
        code = params.code,
        redirect = state.twitch_redirect,
    );

    let token_req = Request::post(req_uri).body(Body::empty())?;

    let response = state
        .client
        .request(token_req)
        .await
        .map_err(AppError::TwitchResponse)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(AppError::ResponseBytes)?;

    let token: TwitchOAuthToken =
        serde_json::from_slice(&bytes).map_err(AppError::DeserializeTwitch)?;

    let req_builder = Request::get(TWITCH_USERS_ENDPOINT)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .header("Client-ID", &state.twitch_client_id);

    let user_req = req_builder.body(Body::empty())?;

    let response = state
        .client
        .request(user_req)
        .await
        .map_err(AppError::TwitchResponse)?;

    let bytes = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(AppError::ResponseBytes)?;

    let user = serde_json::from_slice::<TwitchDataList<TwitchUser>>(&bytes)
        .map_err(AppError::DeserializeTwitch)?
        .data
        .pop()
        .ok_or(AppError::EmptyTwitchData)?;

    let render_data = RenderData {
        status: RenderDataStatus::Success,
        kind: RenderDataKind::Twitch,
        name: &user.display_name,
    };

    let page = state.handlebars.render("auth", &render_data)?;

    info!(
        "Successful twitch authorization for `{}`",
        user.display_name
    );

    state.standby.process_twitch(user, params.state);

    Ok(page)
}
