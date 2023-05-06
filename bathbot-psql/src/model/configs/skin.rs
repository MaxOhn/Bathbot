pub struct DbSkinEntry {
    pub user_id: i32,
    pub username: String,
    pub skin_url: Option<String>,
}

impl From<DbSkinEntry> for SkinEntry {
    fn from(entry: DbSkinEntry) -> Self {
        Self {
            user_id: entry.user_id as u32,
            username: entry.username.into_boxed_str(),
            skin_url: entry.skin_url.expect("query ensures Some").into_boxed_str(),
        }
    }
}

pub struct SkinEntry {
    pub user_id: u32,
    pub username: Box<str>,
    pub skin_url: Box<str>,
}
