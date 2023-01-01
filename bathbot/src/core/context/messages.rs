use eyre::{Result, WrapErr};
use twilight_model::{
    channel::Message,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use crate::Context;

impl Context {
    pub async fn retrieve_channel_history(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> Result<Vec<Message>> {
        self.http
            .channel_messages(channel_id)
            .limit(50)
            .unwrap()
            .exec()
            .await
            .wrap_err("failed to request channel messages")?
            .models()
            .await
            .wrap_err("failed to deserialize channel messages")
    }

    /// Store a message id to register whether the message is not yet
    /// deleted on a later point when calling `remove_msg`.
    pub fn store_msg(&self, msg: Id<MessageMarker>) {
        self.data.msgs_to_process.lock().insert(msg);
    }

    /// Returns false if either `store_msg` was not called for the message id
    /// or if the message was deleted between the `store_msg` call and this call.
    pub fn remove_msg(&self, msg: Id<MessageMarker>) -> bool {
        self.data.msgs_to_process.lock().remove(&msg)
    }

    #[cold]
    pub fn clear_msgs_to_process(&self) {
        self.data.msgs_to_process.lock().clear();
    }
}
