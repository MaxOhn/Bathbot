use std::fmt::Write;

use rosu_v2::prelude::GameMode;
use sqlx::FromRow;

#[derive(FromRow)]
pub struct DbBgGameScore {
    pub discord_id: i64,
    pub score: i32,
}

#[derive(FromRow)]
pub struct DbMapTagEntry {
    pub mapset_id: i32,
    pub image_filename: String,
}

macro_rules! define_map_tags {
    ( $( $column:ident ,)* ) => {
        pub struct DbMapTagsParams {
            pub mode: GameMode,
            $( pub $column: Option<bool>, )*
        }

        impl DbMapTagsParams {
            pub fn new(mode: GameMode) -> Self {
                Self {
                    mode,
                    $( $column: None, )*
                }
            }

            pub(crate) fn into_query(self) -> String{
                let mut query = format!(r#"
SELECT 
  mapset_id, 
  image_filename 
FROM 
  map_tags 
WHERE 
  gamemode = {}"#, self.mode as u8);

                $(
                    if let Some(boolean) = self.$column {
                        let _ = write!(query, " {column} = {boolean}", column = stringify!($column));
                    }
                )*

                query
            }
        }
    };
}

define_map_tags! {
    farm,
    alternate,
    streams,
    old,
    meme,
    hardname,
    kpop,
    english,
    bluesky,
    weeb,
    tech,
    easy,
    hard,
}
