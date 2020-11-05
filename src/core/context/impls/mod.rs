mod bg_game;
mod clients;
mod guilds;
mod links;
mod pp_stars;
mod shutdown;
mod twitch;

use crate::{Context, OsuTracking};

use twilight_model::{
    channel::{Message, Reaction},
    id::RoleId,
};

impl Context {
    /// Returns whether a message was sent by us
    pub fn is_own(&self, other: &Message) -> bool {
        self.cache
            .current_user()
            .map_or(false, |user| user.id == other.author.id)
    }

    pub fn get_role_assign(&self, reaction: &Reaction) -> Option<RoleId> {
        self.data
            .role_assigns
            .get(&(reaction.channel_id.0, reaction.message_id.0))
            .map(|guard| RoleId(*guard.value()))
    }

    pub fn tracking(&self) -> &OsuTracking {
        &self.data.osu_tracking
    }
}
