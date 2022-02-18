mod models;
mod notif_loop;

use std::{borrow::Cow, convert::TryFrom, fmt};

use leaky_bucket_lite::LeakyBucket;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Response,
};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, Duration};

use crate::{
    error::TwitchError,
    util::constants::{
        common_literals::{SORT, USER_ID},
        TWITCH_OAUTH, TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT, TWITCH_VIDEOS_ENDPOINT,
    },
};

pub use self::{models::*, notif_loop::twitch_loop};

type TwitchResult<T> = Result<T, TwitchError>;

pub struct Twitch {
    client: Client,
    auth_token: OAuthToken,
    ratelimiter: LeakyBucket,
}

impl Twitch {
    pub async fn new(client_id: &str, token: &str) -> TwitchResult<Self> {
        let mut headers = HeaderMap::new();
        let client_id_header = HeaderName::try_from("Client-ID").unwrap();
        headers.insert(client_id_header, HeaderValue::from_str(client_id)?);
        let client = Client::builder().default_headers(headers).build()?;

        let form = &[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", token),
        ];

        let auth_response = client.post(TWITCH_OAUTH).form(form).send().await?;
        let bytes = auth_response.bytes().await?;

        let auth_token = serde_json::from_slice(&bytes).map_err(|source| {
            let content = String::from_utf8_lossy(&bytes).into_owned();

            TwitchError::SerdeToken { source, content }
        })?;

        let ratelimiter = LeakyBucket::builder()
            .max(5)
            .tokens(5)
            .refill_interval(Duration::from_millis(200))
            .refill_amount(1)
            .build();

        Ok(Self {
            client,
            auth_token,
            ratelimiter,
        })
    }

    async fn send_request<T: Serialize + ?Sized>(
        &self,
        endpoint: &str,
        data: &T,
    ) -> TwitchResult<Response> {
        self.ratelimiter.acquire_one().await;

        self.client
            .get(endpoint)
            .bearer_auth(&self.auth_token)
            .query(data)
            .send()
            .await
            .map_err(TwitchError::Reqwest)
    }

    pub async fn get_user(&self, name: &str) -> TwitchResult<Option<TwitchUser>> {
        let data = [("login", name)];
        let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
        let bytes = response.bytes().await?;

        let mut users: TwitchData<TwitchUser> =
            serde_json::from_slice(&bytes).map_err(|source| {
                let content = String::from_utf8_lossy(&bytes).into_owned();

                TwitchError::SerdeUser { source, content }
            })?;

        Ok(users.data.pop())
    }

    pub async fn get_user_by_id(&self, user_id: u64) -> TwitchResult<Option<TwitchUser>> {
        let data = [("id", user_id)];
        let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
        let bytes = response.bytes().await?;

        let mut users: TwitchData<TwitchUser> =
            serde_json::from_slice(&bytes).map_err(|source| {
                let content = String::from_utf8_lossy(&bytes).into_owned();

                TwitchError::SerdeUser { source, content }
            })?;

        Ok(users.data.pop())
    }

    pub async fn get_users(&self, user_ids: &[u64]) -> TwitchResult<Vec<TwitchUser>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut users = Vec::with_capacity(user_ids.len());

        for chunk in user_ids.chunks(100) {
            let data: Vec<_> = chunk.iter().map(|&id| ("id", id)).collect();
            let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
            let bytes = response.bytes().await?;

            let parsed_response: TwitchData<TwitchUser> =
                serde_json::from_slice(&bytes).map_err(|source| {
                    let content = String::from_utf8_lossy(&bytes).into_owned();

                    TwitchError::SerdeUsers { source, content }
                })?;

            users.extend(parsed_response.data);
        }

        Ok(users)
    }

    pub async fn get_streams(&self, user_ids: &[u64]) -> TwitchResult<Vec<TwitchStream>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut streams = Vec::with_capacity(user_ids.len());
        let mut interval = interval(Duration::from_millis(1000));

        for chunk in user_ids.chunks(100) {
            interval.tick().await;
            let mut data: Vec<_> = chunk.iter().map(|&id| (USER_ID, id)).collect();
            data.push(("first", chunk.len() as u64));
            let response = self.send_request(TWITCH_STREAM_ENDPOINT, &data).await?;
            let bytes = response.bytes().await?;

            let parsed_response: TwitchData<TwitchStream> = serde_json::from_slice(&bytes)
                .map_err(|source| {
                    let content = String::from_utf8_lossy(&bytes).into_owned();

                    TwitchError::SerdeStreams { source, content }
                })?;

            streams.extend(parsed_response.data);
        }

        Ok(streams)
    }

    pub async fn get_last_vod(&self, user_id: u64) -> TwitchResult<Option<TwitchVideo>> {
        let data = [
            (USER_ID, Cow::Owned(user_id.to_string())),
            ("first", "1".into()),
            (SORT, "time".into()),
        ];

        let response = self.send_request(TWITCH_VIDEOS_ENDPOINT, &data).await?;
        let bytes = response.bytes().await?;

        let mut videos: TwitchData<TwitchVideo> =
            serde_json::from_slice(&bytes).map_err(|source| {
                let content = String::from_utf8_lossy(&bytes).into_owned();

                TwitchError::SerdeVideos { source, content }
            })?;

        Ok(videos.data.pop())
    }
}

#[derive(Deserialize)]
pub struct OAuthToken {
    access_token: String,
}

impl fmt::Display for OAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.access_token)
    }
}
