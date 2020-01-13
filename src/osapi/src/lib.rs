#![allow(dead_code)]

pub mod backend;
pub mod models;
pub mod util;

#[macro_use]
extern crate log;
extern crate futures;
extern crate hyper;
extern crate tokio;

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused)]
    use super::{
        backend::{requests::*, Osu, OsuError},
        models::*,
        util::*,
    };
    use chrono::Utc;
    use std::env;
    use tokio::runtime::Runtime;

    #[test]
    fn get_user() {
        let mut rt = Runtime::new().unwrap();
        rt.block_on(async move {
            kankyo::load().expect("Could not read .env file");
            env_logger::init();
            let osu_key = env::var("OSU_TOKEN").expect("Could not find env variable 'OSU_TOKEN'");
            let osu = Osu::new(osu_key);
            let mut req = UserReq::new();
            req.username("Badewanne3".to_owned());
            let user_future = osu.get_user(req);
            match user_future.await {
                Ok(user) => {
                    println!(
                        "Name: {}, ranked score: {}",
                        user.username, user.ranked_score
                    );
                },
                Err(e) => eprintln!("Error while retrieving user: {:?}", e),
            }
        });
    }

    #[test]
    #[ignore]
    fn test_ratelimiter() {
        let start = Utc::now().timestamp_millis();
        let mut ratelimiter = RateLimiter::new(500, 7);
        let mut counter = 0;
        while counter < 53 {
            ratelimiter.wait_access();
            counter += 1;
        }
        let end = Utc::now().timestamp_millis();
        let diff = end - start;
        // Make sure the limiter actually waits to grant access but doesn't take too long
        assert!(diff < 5000 && diff > 3500);
    }
}
