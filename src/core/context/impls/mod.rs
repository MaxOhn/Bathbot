mod bg_game;
mod clients;
mod guilds;
mod links;
mod pp_stars;
mod shutdown;
mod twitch;

use crate::Context;

use twilight::model::{
    channel::{Message, Reaction},
    id::RoleId,
};

impl Context {
    /// Returns if a message was sent by us
    pub fn is_own(&self, other: &Message) -> bool {
        self.cache.bot_user.id == other.author.id
    }

    pub fn get_role_assign(&self, reaction: &Reaction) -> Option<RoleId> {
        self.data
            .role_assigns
            .get(&(reaction.channel_id.0, reaction.message_id.0))
            .map(|guard| RoleId(*guard.value()))
    }
}
