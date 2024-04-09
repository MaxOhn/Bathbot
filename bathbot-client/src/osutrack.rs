use eyre::{Result, WrapErr};
use http::{header::USER_AGENT, Method, Request};
use hyper::Body;
use rosu_v2::model::GameMode;

use crate::{Client, ClientError, MY_USER_AGENT};

impl Client {
    pub async fn notify_osutrack_user_activity(
        &self,
        user_id: u32,
        mode: GameMode,
    ) -> Result<(), ClientError> {
        let url = format!(
            "https://osutrack-api.ameo.dev/update?user={user_id}&mode={mode}",
            mode = mode as u8
        );

        trace!("POST simple request to url {url}");

        let req = Request::builder()
            .method(Method::POST)
            .uri(&url)
            .header(USER_AGENT, MY_USER_AGENT)
            .body(Body::empty())
            .wrap_err("Failed to build POST request")?;

        let response = self
            .client
            .request(req)
            .await
            .wrap_err("Failed to receive POST response")?;

        let status = response.status();

        match status.as_u16() {
            200..=299 => {}
            400 => return Err(ClientError::BadRequest),
            404 => return Err(ClientError::NotFound),
            429 => return Err(ClientError::Ratelimited),
            _ => {
                let err = eyre!("Failed with status code {status} when requesting url {url}");

                return Err(err.into());
            }
        };

        Ok(())
    }
}
