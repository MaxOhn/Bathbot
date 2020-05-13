use super::models::{TwitchStream, TwitchStreams, TwitchUser, TwitchUsers};
use crate::util::{
    globals::{TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT},
    Error, RateLimiter,
};

use rayon::prelude::*;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Response,
};
use serde::Serialize;
use serde_derive::Deserialize;
use std::{convert::TryFrom, fmt, sync::Mutex};

pub struct Twitch {
    client: Client,
    auth_token: OAuthToken,
    twitch_limiter: Mutex<RateLimiter>,
}

impl Twitch {
    pub async fn new(client_id: &str, token: &str) -> Result<Self, Error> {
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
        let auth_token = serde_json::from_slice(&auth_response.bytes().await?)?;
        Ok(Self {
            client,
            auth_token,
            twitch_limiter: Mutex::new(RateLimiter::new(5, 1)),
        })
    }

    async fn send_request<T: Serialize + ?Sized>(
        &self,
        endpoint: &str,
        data: &T,
    ) -> Result<Response, reqwest::Error> {
        {
            self.twitch_limiter
                .lock()
                .unwrap_or_else(|why| panic!("Could not lock twitch_limiter: {}", why))
                .await_access();
        }
        self.client
            .get(endpoint)
            .bearer_auth(&self.auth_token)
            .query(data)
            .send()
            .await
    }

    pub async fn get_user(&self, name: &str) -> Result<TwitchUser, Error> {
        let data = vec![("login", name)];
        let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
        let mut users: TwitchUsers = serde_json::from_slice(&response.bytes().await?)?;
        match users.data.pop() {
            Some(user) => Ok(user),
            None => Err(Error::Custom(format!(
                "Twitch API gave no results for username {}",
                name
            ))),
        }
    }

    pub async fn get_users(&self, user_ids: &[u64]) -> Result<Vec<TwitchUser>, Error> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut users = Vec::with_capacity(user_ids.len());
        for chunk in user_ids.chunks(100) {
            let data: Vec<_> = chunk.par_iter().map(|&id| ("id", id)).collect();
            let response = self.send_request(TWITCH_USERS_ENDPOINT, &data).await?;
            let parsed_response: TwitchUsers = serde_json::from_slice(&response.bytes().await?)?;
            users.extend(parsed_response.data);
        }
        Ok(users)
    }

    pub async fn get_streams(&self, user_ids: &[u64]) -> Result<Vec<TwitchStream>, Error> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut streams = Vec::with_capacity(user_ids.len());
        for chunk in user_ids.chunks(100) {
            let mut data: Vec<_> = chunk.par_iter().map(|&id| ("user_id", id)).collect();
            data.push(("first", user_ids.len() as u64));
            let response = self.send_request(TWITCH_STREAM_ENDPOINT, &data).await?;
            let parsed_response: TwitchStreams = serde_json::from_slice(&response.bytes().await?)?;
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
        write!(f, "{}", self.access_token)
    }
}
