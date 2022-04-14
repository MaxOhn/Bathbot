use dashmap::mapref::entry::Entry;
use twilight_model::{
    channel::Reaction,
    id::{
        marker::{ChannelMarker, MessageMarker, RoleMarker},
        Id,
    },
};

use crate::Context;

use super::AssignRoles;

impl Context {
    pub fn get_role_assigns(&self, reaction: &Reaction) -> Option<AssignRoles> {
        self.data
            .role_assigns
            .get(&(reaction.channel_id.get(), reaction.message_id.get()))
            .map(|guard| guard.value().to_owned())
    }

    #[cold]
    pub fn add_role_assign(
        &self,
        channel_id: Id<ChannelMarker>,
        msg_id: Id<MessageMarker>,
        role_id: Id<RoleMarker>,
    ) {
        let role_id = role_id.get();

        let mut roles = self
            .data
            .role_assigns
            .entry((channel_id.get(), msg_id.get()))
            .or_default();

        if !roles.contains(&role_id) {
            roles.push(role_id);
        }
    }

    #[cold]
    pub fn remove_role_assign(
        &self,
        channel_id: Id<ChannelMarker>,
        msg_id: Id<MessageMarker>,
        role_id: Id<RoleMarker>,
    ) {
        let entry = self
            .data
            .role_assigns
            .entry((channel_id.get(), msg_id.get()));

        if let Entry::Occupied(mut e) = entry {
            let role_id = role_id.get();
            e.get_mut().retain(|r| *r != role_id);

            if e.get().is_empty() {
                e.remove();
            }
        }
    }
}
