use std::{
    borrow::Cow,
    fmt::{Display, Write},
    time::Duration,
};

use bathbot_model::{TwitchDataList, TwitchStream, TwitchUser, TwitchVideo};
use bathbot_util::constants::{
    TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT, TWITCH_VIDEOS_ENDPOINT,
};
use bytes::Bytes;
use eyre::{Result, WrapErr};
use tokio::time::interval;

use crate::{Client, ClientError, Site};

#[cfg(feature = "twitch")]
impl Client {
    #[cfg(feature = "twitch")]
    pub(crate) async fn get_twitch_token(
        client: &crate::client::InnerClient,
        client_id: &str,
        token: &str,
    ) -> Result<bathbot_model::TwitchData> {
        use bathbot_model::TwitchData;
        use bathbot_util::constants::TWITCH_OAUTH;
        use http::{
            header::{CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT},
            Method, Request,
        };
        use hyper::Body;

        use crate::{multipart::Multipart, MY_USER_AGENT};

        let mut form = Multipart::new();

        form.push_text("grant_type", "client_credentials")
            .push_text("client_id", client_id)
            .push_text("client_secret", token);

        let client_id = http::HeaderValue::from_str(client_id)?;
        let content_type = form.content_type();
        let content = form.build();

        let req = Request::builder()
            .method(Method::POST)
            .uri(TWITCH_OAUTH)
            .header(USER_AGENT, MY_USER_AGENT)
            .header("Client-ID", client_id.clone())
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, content.len())
            .body(Body::from(content))
            .wrap_err("Failed to build POST request")?;

        let response = client.request(req).await?;
        let bytes = Self::error_for_status(response, TWITCH_OAUTH).await?;

        let oauth_token = serde_json::from_slice(&bytes).wrap_err_with(|| {
            let body = String::from_utf8_lossy(&bytes);

            format!("Failed to deserialize twitch token: {body}")
        })?;

        Ok(TwitchData {
            client_id,
            oauth_token,
        })
    }
}

impl Client {
    async fn make_twitch_get_request<I, U, V>(
        &self,
        url: impl AsRef<str>,
        data: I,
    ) -> Result<Bytes, ClientError>
    where
        I: IntoIterator<Item = (U, V)>,
        U: Display,
        V: Display,
    {
        let url = url.as_ref();

        let mut uri = format!("{url}?");
        let mut iter = data.into_iter();

        if let Some((key, value)) = iter.next() {
            let _ = write!(uri, "{key}={value}");

            for (key, value) in iter {
                let _ = write!(uri, "&{key}={value}");
            }
        }

        self.make_get_request(&uri, Site::Twitch).await
    }

    pub async fn get_twitch_user(&self, name: &str) -> Result<Option<TwitchUser>> {
        let data = [("login", name)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
            .await?;

        let mut users: TwitchDataList<TwitchUser> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize twitch username: {body}")
            })?;

        Ok(users.data.pop())
    }

    pub async fn get_twitch_user_by_id(&self, user_id: u64) -> Result<Option<TwitchUser>> {
        let data = [("id", user_id)];

        let bytes = self
            .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
            .await?;

        let mut users: TwitchDataList<TwitchUser> =
            serde_json::from_slice(&bytes).wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize twitch user id: {body}")
            })?;

        Ok(users.data.pop())
    }

    pub async fn get_twitch_users(&self, user_ids: &[u64]) -> Result<Vec<TwitchUser>> {
        let mut users = Vec::with_capacity(user_ids.len());

        for chunk in user_ids.chunks(100) {
            let data: Vec<_> = chunk.iter().map(|&id| ("id", id)).collect();

            let bytes = self
                .make_twitch_get_request(TWITCH_USERS_ENDPOINT, data)
                .await?;

            let mut parsed_response: TwitchDataList<TwitchUser> = serde_json::from_slice(&bytes)
                .wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("Failed to deserialize twitch users: {body}")
                })?;

            users.append(&mut parsed_response.data);
        }

        Ok(users)
    }

    pub async fn get_twitch_stream(&self, user_id: u64) -> Result<Option<TwitchStream>> {
        let data = [("user_id", user_id)];

        let bytes = self
            .make_twitch_get_request(TWITCH_STREAM_ENDPOINT, data)
            .await?;

        let mut streams: TwitchDataList<TwitchStream> = serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize twitch stream: {body}")
            })?;

        Ok(streams.data.pop())
    }

    pub async fn get_twitch_streams(&self, user_ids: &[u64]) -> Result<Vec<TwitchStream>> {
        let mut streams = Vec::with_capacity(user_ids.len());
        let mut interval = interval(Duration::from_millis(1000));

        for chunk in user_ids.chunks(100) {
            interval.tick().await;
            let mut data: Vec<_> = chunk.iter().map(|&id| ("user_id", id)).collect();
            data.push(("first", chunk.len() as u64));

            let bytes = self
                .make_twitch_get_request(TWITCH_STREAM_ENDPOINT, data)
                .await?;

            let mut parsed_response: TwitchDataList<TwitchStream> = serde_json::from_slice(&bytes)
                .wrap_err_with(|| {
                    let body = String::from_utf8_lossy(&bytes);

                    format!("Failed to deserialize twitch streams: {body}")
                })?;

            streams.append(&mut parsed_response.data);
        }

        Ok(streams)
    }

    pub async fn get_last_twitch_vod(&self, user_id: u64) -> Result<Option<TwitchVideo>> {
        let data = [
            ("user_id", Cow::Owned(user_id.to_string())),
            ("first", "1".into()),
            ("sort", "time".into()),
            ("type", "archive".into()),
        ];

        let bytes = self
            .make_twitch_get_request(TWITCH_VIDEOS_ENDPOINT, data)
            .await?;

        let mut videos: TwitchDataList<TwitchVideo> = serde_json::from_slice(&bytes)
            .wrap_err_with(|| {
                let body = String::from_utf8_lossy(&bytes);

                format!("Failed to deserialize twitch videos: {body}")
            })?;

        Ok(videos.data.pop())
    }
}
