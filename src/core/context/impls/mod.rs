mod background_loop;
mod configs;
mod game_states;
mod match_live;
mod shutdown;
mod twitch;

pub use background_loop::GarbageCollectMap;
pub use match_live::{MatchLiveChannels, MatchTrackResult};

use dashmap::mapref::entry::Entry;
use twilight_http::client::InteractionClient;
use twilight_model::{
    channel::{Message, Reaction},
    id::{
        marker::{ChannelMarker, MessageMarker, RoleMarker},
        Id,
    },
};

use crate::{util::CountryCode, BotResult, Context, OsuTracking};

use super::AssignRoles;

impl Context {
    /// Returns whether a message was sent by us
    pub async fn is_own(&self, other: &Message) -> bool {
        match self.cache.current_user() {
            Ok(user) => user.id == other.author.id,
            Err(_) => false,
        }
    }

    pub fn interaction(&self) -> InteractionClient<'_> {
        self.http.interaction(self.data.application_id)
    }

    pub fn get_role_assigns(&self, reaction: &Reaction) -> Option<AssignRoles> {
        self.data
            .role_assigns
            .get(&(reaction.channel_id.get(), reaction.message_id.get()))
            .map(|guard| guard.value().to_owned())
    }

    pub fn tracking(&self) -> &OsuTracking {
        &self.data.osu_tracking
    }

    pub async fn retrieve_channel_history(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> BotResult<Vec<Message>> {
        self.http
            .channel_messages(channel_id)
            .limit(50)
            .unwrap()
            .exec()
            .await?
            .models()
            .await
            .map_err(|e| e.into())
    }

    /// Store a message id to register whether the message is not yet
    /// deleted on a later point when calling `remove_msg`.
    pub fn store_msg(&self, msg: Id<MessageMarker>) {
        self.data.msgs_to_process.insert(msg);
    }

    /// Returns false if either `store_msg` was not called for the message id
    /// or if the message was deleted between the `store_msg` call and this call.
    pub fn remove_msg(&self, msg: Id<MessageMarker>) -> bool {
        self.data.msgs_to_process.remove(&msg).is_some()
    }

    #[cold]
    pub fn clear_msgs_to_process(&self) {
        self.data.msgs_to_process.clear();
    }

    pub fn add_country(&self, country: String, code: CountryCode) {
        self.data.snipe_countries.insert(code, country);
    }

    pub fn contains_country(&self, code: &str) -> bool {
        self.data.snipe_countries.contains_key(code)
    }

    pub fn get_country(&self, code: &str) -> Option<String> {
        self.data
            .snipe_countries
            .get(code)
            .map(|entry| entry.value().to_owned())
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
