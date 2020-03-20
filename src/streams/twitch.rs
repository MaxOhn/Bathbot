use super::models::{TwitchStream, TwitchStreams, TwitchUser, TwitchUsers};
use crate::util::{
    globals::{TWITCH_STREAM_ENDPOINT, TWITCH_USERS_ENDPOINT},
    Error, RateLimiter,
};

use rayon::prelude::*;
use reqwest::{
    header::{self, HeaderMap, HeaderName},
    Client,
};
use std::{convert::TryFrom, sync::Mutex};

pub struct Twitch {
    client: Client,
    twitch_limiter: Mutex<RateLimiter>,
}

impl Twitch {
    pub fn new(client_id: &str, token: &str) -> Result<Self, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(token).unwrap(),
        );
        let client_id_header = HeaderName::try_from("Client-ID").unwrap();
        headers.insert(
            client_id_header,
            header::HeaderValue::from_str(client_id).unwrap(),
        );
        let client = Client::builder().default_headers(headers).build()?;
        Ok(Self {
            client,
            twitch_limiter: Mutex::new(RateLimiter::new(5, 1)),
        })
    }

    pub async fn get_user(&self, name: &str) -> Result<TwitchUser, Error> {
        let data = vec![("login", name)];
        {
            self.twitch_limiter
                .lock()
                .expect("Could not lock twitch_limiter for users")
                .await_access();
        }
        let response = self
            .client
            .get(TWITCH_USERS_ENDPOINT)
            .query(&data)
            .send()
            .await?;
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
        } else if user_ids.len() > 100 {
            return Err(Error::Custom(format!(
                "user_ids len must be at most 100, got {}",
                user_ids.len()
            )));
        }
        let data: Vec<_> = user_ids.par_iter().map(|&id| ("id", id)).collect();
        {
            self.twitch_limiter
                .lock()
                .expect("Could not lock twitch_limiter for users")
                .await_access();
        }
        let response = self
            .client
            .get(TWITCH_USERS_ENDPOINT)
            .query(&data)
            .send()
            .await?;
        let users: TwitchUsers = serde_json::from_slice(&response.bytes().await?)?;
        Ok(users.data)
    }

    pub async fn get_streams(&self, user_ids: &[u64]) -> Result<Vec<TwitchStream>, Error> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        } else if user_ids.len() > 100 {
            return Err(Error::Custom(format!(
                "user_ids len must be at most 100, got {}",
                user_ids.len()
            )));
        }
        let mut data: Vec<_> = user_ids.par_iter().map(|&id| ("user_id", id)).collect();
        data.push(("first", user_ids.len() as u64));
        {
            self.twitch_limiter
                .lock()
                .expect("Could not lock twitch_limiter for streams")
                .await_access();
        }
        let response = self
            .client
            .get(TWITCH_STREAM_ENDPOINT)
            .query(&data)
            .send()
            .await?;
        let streams: TwitchStreams = serde_json::from_slice(&response.bytes().await?)?;
        Ok(streams.data)
    }
}
