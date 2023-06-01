use bathbot_psql::{model::osu::MapBookmark, Database};
use eyre::{Result, WrapErr};
use twilight_model::id::{marker::UserMarker, Id};

#[derive(Copy, Clone)]
pub struct BookmarkManager<'d> {
    psql: &'d Database,
}

impl<'d> BookmarkManager<'d> {
    pub fn new(psql: &'d Database) -> Self {
        Self { psql }
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
