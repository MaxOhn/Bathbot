use bathbot_psql::{Database, model::osu::MapBookmark};
use eyre::{Result, WrapErr};
use twilight_model::id::{Id, marker::UserMarker};

use crate::core::Context;

#[derive(Copy, Clone)]
pub struct BookmarkManager {
    psql: &'static Database,
}

impl BookmarkManager {
    pub fn new() -> Self {
        Self {
            psql: Context::psql(),
        }
    }

    pub async fn get(self, user: Id<UserMarker>) -> Result<Vec<MapBookmark>> {
        self.psql
            .select_user_bookmarks(user)
            .await
            .wrap_err("Failed to get bookmarks")
    }

    pub async fn add(self, user: Id<UserMarker>, map_id: u32) -> Result<()> {
        self.psql
            .insert_user_bookmark(user, map_id)
            .await
            .wrap_err("Failed to insert user bookmark")
    }

    pub async fn remove(self, user: Id<UserMarker>, map_id: u32) -> Result<()> {
        self.psql
            .delete_user_bookmark(user, map_id)
            .await
            .wrap_err("Failed to delete user bookmark")
    }
}
