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
            .pin()
            .get(&(reaction.channel_id.get(), reaction.message_id.get()))
            .map(AssignRoles::to_owned)
    }

    #[cold]
    pub fn add_role_assign(
        &self,
        channel_id: Id<ChannelMarker>,
        msg_id: Id<MessageMarker>,
        role_id: Id<RoleMarker>,
    ) {
        let role_id = role_id.get();
        let key = (channel_id.get(), msg_id.get());

        let assigns = &self.data.role_assigns;
        let guard = assigns.guard();

        let missing = assigns
            .compute_if_present(
                &key,
                |_, roles| {
                    let mut roles = roles.to_owned();

                    if !roles.contains(&role_id) {
                        roles.push(role_id);
                    }

                    Some(roles)
                },
                &guard,
            )
            .is_none();

        if missing {
            let roles = smallvec::smallvec![role_id];
            self.data.role_assigns.insert(key, roles, &guard);
        }
    }

    #[cold]
    pub fn remove_role_assign(
        &self,
        channel_id: Id<ChannelMarker>,
        msg_id: Id<MessageMarker>,
        role_id: Id<RoleMarker>,
    ) {
        let role_id = role_id.get();
        let key = (channel_id.get(), msg_id.get());

        self.data
            .role_assigns
            .pin()
            .compute_if_present(&key, |_, roles| {
                if roles.contains(&role_id) {
                    if roles.len() == 1 {
                        return None;
                    }

                    let mut roles = roles.to_owned();
                    roles.retain(|r| *r != role_id);

                    Some(roles)
                } else {
                    Some(roles.to_owned())
                }
            });
    }
}
