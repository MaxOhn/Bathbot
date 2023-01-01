use sqlx::FromRow;

#[derive(FromRow)]
pub struct DbHlGameScore {
    pub discord_id: i64,
    pub highscore: i32,
}
