use crate::{backend::requests::*, models::*, util::RateLimiter};

use futures::{
    future::{ok, Either},
    Future, TryFutureExt, TryStreamExt,
};
use hyper::{
    body::Bytes,
    client::{connect::dns::GaiResolver, HttpConnector},
    http::uri::InvalidUri,
    Body, Client, Request, Response, Uri,
};
use hyper_tls::HttpsConnector;
use serde::de::DeserializeOwned;
use std::{
    char,
    collections::HashMap,
    fmt::Debug,
    string::FromUtf8Error,
    sync::{Arc, Mutex},
};

const API_BASE: &'static str = "https://osu.ppy.sh/api/";
const USER: &'static str = "get_user";

type Cache<K = Uri, V = String> = Arc<Mutex<HashMap<K, V>>>;

pub struct Osu {
    client: Client<HttpsConnector<HttpConnector<GaiResolver>>, Body>,
    api_key: String,
    ratelimiter: RateLimiter,
    cache: Cache,
}

impl Osu {
    pub fn new(api_key: impl AsRef<str>) -> Self {
        let https = HttpsConnector::new();
        Osu {
            client: Client::builder().build::<_, Body>(https),
            api_key: api_key.as_ref().to_owned(),
            ratelimiter: RateLimiter::new(1000, 10),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_user(&self, req: UserReq) -> Result<User, OsuError> {
        if req.user_id.is_none() && req.username.is_none() {
            return Err(OsuError::ReqBuilder(
                "Neither user id nor username were specified for retrieving a user from the osu! API".to_owned()
            ));
        }
        let mut url = format!("{}{}?k={}&u=", API_BASE, USER, self.api_key);
        if let Some(username) = req.username {
            url.push_str(&username);
        } else if let Some(user_id) = req.user_id {
            url.push_str(&user_id.to_string());
        }
        if let Some(mode) = req.mode {
            url.push_str("&m=");
            url.push(char::from_digit(mode as u32, 10).ok_or_else(|| {
                OsuError::ReqBuilder(format!("Could not parse mode {} into char", mode as u32))
            })?);
        }
        self.cached_resp(url.parse()?).await
        //Ok(User::default())
    }

    /// Util function that either returns deserialized response from cache or fetches response from url and then deserializes it
    pub(crate) fn cached_resp<T: Debug + DeserializeOwned>(
        &self,
        url: Uri,
    ) -> impl Future<Output = Result<T, OsuError>> {
        let maybe_response: Option<T> = self
            .cache
            .lock()
            .unwrap()
            .get(&url)
            .map(|response| serde_json::from_str(response).unwrap());
        if let Some(response) = maybe_response {
            debug!("Found cached: {:?}", response);
            Either::Left(ok(response))
        } else {
            debug!("Nothing in cache. Fetching...");
            let url2 = url.clone();
            let req = Request::builder().uri(url).body(Body::from("")).unwrap();
            let do_request = self.client.request(req);
            let cache = self.cache.clone();
            Either::Right(
                do_request
                    .and_then(|response: Response<Body>| {
                        let body: Body = response.into_body();
                        body.try_collect::<Vec<Bytes>>()
                    })
                    .map_ok(move |chunk| {
                        println!("{:?}", chunk);
                        let x = chunk.iter().flat_map(|bytes| bytes.slice(..)).collect::<Vec<u8>>();
                        println!("{:?}", x);
                        let maybe_response = String::from_utf8(
                            //chunk.downcast::<Parts<Body>>().unwrap().read_buf.to_vec(),
                            chunk
                                .iter()
                                .flat_map(|bytes| bytes.slice(..))
                                .collect::<Vec<u8>>(),
                        );
                        let string_response = match maybe_response {
                            Ok(response) => response,
                            Err(e) => panic!("Error while parsing Bytes to string: {}", e),
                        };
                        /*
                        let string_response = String::from_utf8(
                            //chunk.downcast::<Parts<Body>>().unwrap().read_buf.to_vec(),
                            chunk
                                .iter()
                                .flat_map(|bytes| bytes.slice(..))
                                .collect::<Vec<u8>>(),
                        )
                        .unwrap();
                        */
                        debug!("Deserializing...");
                        let deserialized: T = serde_json::from_str(&string_response).unwrap();
                        cache.lock().unwrap().insert(url2, string_response);
                        deserialized
                    })
                    .map_err(|e| OsuError::Other(format!("Error while fetching: {}", e))),
            )
        }
    }
}

#[derive(Debug)]
pub enum OsuError {
    ReqBuilder(String),
    Hyper(::hyper::Error),
    Json(::serde_json::Error),
    Uri(InvalidUri),
    FromUtf8(FromUtf8Error),
    Other(String),
}

impl From<::hyper::Error> for OsuError {
    fn from(err: ::hyper::Error) -> Self {
        OsuError::Hyper(err)
    }
}

impl From<::serde_json::Error> for OsuError {
    fn from(err: ::serde_json::Error) -> Self {
        OsuError::Json(err)
    }
}

impl From<InvalidUri> for OsuError {
    fn from(err: InvalidUri) -> Self {
        OsuError::Uri(err)
    }
}

impl From<FromUtf8Error> for OsuError {
    fn from(err: FromUtf8Error) -> Self {
        OsuError::FromUtf8(err)
    }
}
