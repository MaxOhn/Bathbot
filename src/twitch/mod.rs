mod models;
mod notif_loop;

pub use models::*;
pub use notif_loop::twitch_loop;

use crate::util::{
    constants::{TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT},
    error::TwitchError,
};

use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Response,
};
use serde::{Deserialize, Serialize};
use std::{convert::TryFrom, fmt, num::NonZeroU32};
use tokio::time::{interval, Duration};

type TwitchResult<T> = Result<T, TwitchError>;

pub struct Twitch {
    client: Client,
    auth_token: OAuthToken,
    ratelimiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
}

impl Twitch {
    pub async fn new(client_id: &str, token: &str) -> TwitchResult<Self> {
        let mut headers = HeaderMap::new();
        let client_id_header = HeaderName::try_from("Client-ID").unwrap();
        headers.insert(client_id_header, HeaderValue::from_str(client_id)?);
        let client = Client::builder().default_headers(headers).build()?;

        let auth_response = client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id),
                ("client_secret", token),
            ])
            .send()
            .await?;

        let auth_token = serde_json::from_slice(&auth_response.bytes().await?)
            .map_err(TwitchError::InvalidAuth)?;

        let quota = Quota::per_second(NonZeroU32::new(5).unwrap());
        let ratelimiter = RateLimiter::direct(quota);

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
        self.ratelimiter.until_ready().await;

        self.client
            .get(endpoint)
            .bearer_auth(&self.auth_token)
            .query(data)
            .send()
            .await
            .map_err(TwitchError::Reqwest)
    }

    pub async fn get_user(&self, name: &str) -> TwitchResult<TwitchUser> {
        let data = vec![("login", name)];
        let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
        let bytes = response.bytes().await?;

        let mut users: TwitchUsers = serde_json::from_slice(&bytes).map_err(|e| {
            let content = String::from_utf8_lossy(&bytes).into_owned();
            TwitchError::SerdeUser(e, content)
        })?;

        match users.data.pop() {
            Some(user) => Ok(user),
            None => Err(TwitchError::NoUserResult(name.to_string())),
        }
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

            let parsed_response: TwitchUsers = serde_json::from_slice(&bytes).map_err(|e| {
                let content = String::from_utf8_lossy(&bytes).into_owned();
                TwitchError::SerdeUsers(e, content)
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
            let mut data: Vec<_> = chunk.iter().map(|&id| ("user_id", id)).collect();
            data.push(("first", chunk.len() as u64));
            let response = self.send_request(TWITCH_STREAM_ENDPOINT, &data).await?;
            let bytes = response.bytes().await?;

            let parsed_response: TwitchStreams = serde_json::from_slice(&bytes).map_err(|e| {
                let content = String::from_utf8_lossy(&bytes).into_owned();
                TwitchError::SerdeStreams(e, content)
            })?;

            streams.extend(parsed_response.data);
        }

        Ok(streams)
    }
}

#[derive(Deserialize)]
struct OAuthToken {
    access_token: String,
}

impl fmt::Display for OAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.access_token)
    }
}
