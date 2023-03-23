use rkyv::{
    with::{Inline, Map, RefAsBox},
    Archive, Serialize,
};
use twilight_model::{
    guild::{Guild, PartialGuild, Permissions},
    id::{
        marker::{GuildMarker, UserMarker},
        Id,
    },
    util::ImageHash,
};

#[derive(Archive, Serialize)]
pub struct CachedGuild<'g> {
    #[with(Map<Inline>)]
    pub icon: Option<&'g ImageHash>,
    pub id: Id<GuildMarker>,
    #[with(RefAsBox)]
    pub name: &'g str,
    pub owner_id: Id<UserMarker>,
    pub permissions: Option<Permissions>,
}

impl<'g> From<&'g Guild> for CachedGuild<'g> {
    #[inline]
    fn from(guild: &'g Guild) -> Self {
        Self {
            icon: guild.icon.as_ref(),
            id: guild.id,
            name: &guild.name,
            owner_id: guild.owner_id,
            permissions: guild.permissions,
        }
    }
}

impl<'g> From<&'g PartialGuild> for CachedGuild<'g> {
    #[inline]
    fn from(guild: &'g PartialGuild) -> Self {
        Self {
            icon: guild.icon.as_ref(),
            id: guild.id,
            name: &guild.name,
            owner_id: guild.owner_id,
            permissions: guild.permissions,
        }
    }
}
