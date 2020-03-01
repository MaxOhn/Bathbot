use super::super::schema::guilds;
use serenity::{
    framework::standard::{Args, Delimiter},
    model::id::{GuildId, RoleId},
};

#[derive(Insertable, Queryable, Identifiable, Debug, PartialEq)]
#[table_name = "guilds"]
#[primary_key("guild_id")]
pub struct GuildDB {
    pub guild_id: u64,
    with_lyrics: bool,
    authorities: String,
    vc_role: Option<u64>,
}

impl GuildDB {
    pub fn new(
        guild_id: u64,
        with_lyrics: bool,
        authorities: String,
        vc_role: Option<u64>,
    ) -> Self {
        Self {
            guild_id,
            with_lyrics,
            authorities,
            vc_role,
        }
    }
}

impl Into<Guild> for GuildDB {
    fn into(self) -> Guild {
        let mut authorities = Vec::new();
        let mut args = Args::new(&self.authorities, &[Delimiter::Single(' ')]);
        while !args.is_empty() {
            authorities.push(
                args.single_quoted()
                    .expect("Could not unwrap args in Into<Guild>"),
            );
        }
        Guild {
            guild_id: GuildId(self.guild_id),
            with_lyrics: self.with_lyrics,
            authorities,
            vc_role: self.vc_role.map(RoleId),
        }
    }
}

pub struct Guild {
    pub guild_id: GuildId,
    pub with_lyrics: bool,
    pub authorities: Vec<String>,
    pub vc_role: Option<RoleId>,
}
