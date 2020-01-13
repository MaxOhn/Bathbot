pub mod maps;
pub mod scores;
pub mod user;
pub mod user_best;
pub mod user_recent;

pub use maps::MapsReq;
pub use scores::ScoresReq;
pub use user::UserReq;
pub use user_best::UserBestReq;
pub use user_recent::UserRecentReq;

pub trait Request {
    type Output;
    fn queue(&self) -> Self::Output;
}
