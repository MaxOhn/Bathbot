use std::time::Duration;

use bathbot_util::MessageBuilder;
use eyre::{Report, Result, WrapErr};
use tokio::{
    sync::watch::{self, Receiver},
    time::sleep,
};
use twilight_model::id::{
    marker::{ChannelMarker, MessageMarker},
    Id,
};

use super::{
    origin::{ActiveMessageOrigin, ActiveMessageOriginError},
    ActiveMessage, BuildPage, FullActiveMessage, IActiveMessage,
};
use crate::core::Context;

pub struct ActiveMessagesBuilder {
    inner: ActiveMessage,
    attachment: Option<(String, Vec<u8>)>,
    start_by_update: Option<bool>,
}

impl ActiveMessagesBuilder {
    pub fn new(active_msg: impl Into<ActiveMessage>) -> Self {
        Self {
            inner: active_msg.into(),
            attachment: None,
            start_by_update: None,
        }
    }

    pub async fn begin(self, orig: impl Into<ActiveMessageOrigin<'_>>) -> Result<()> {
        self.begin_with_err(orig).await.map_err(|err| match err {
            ActiveMessageOriginError::Report(err) => err,
            err @ ActiveMessageOriginError::CannotDmUser => Report::new(err),
        })
    }

    pub async fn begin_with_err(
        self,
        orig: impl Into<ActiveMessageOrigin<'_>>,
    ) -> Result<(), ActiveMessageOriginError> {
        let Self {
            inner: mut active_msg,
            attachment,
            start_by_update,
        } = self;

        let BuildPage {
            embed,
            content,
            defer: _,
        } = active_msg
            .build_page()
            .await
            .wrap_err("Failed to build page")?;

        let components = active_msg.build_components();

        let mut builder = MessageBuilder::new().embed(embed).components(components);

        if let Some(ref content) = content {
            builder = builder.content(content.as_ref());
        }

        if let Some((name, bytes)) = attachment {
            builder = builder.attachment(name, bytes);
        }

        let orig: ActiveMessageOrigin<'_> = orig.into();

        let response_raw = if start_by_update.unwrap_or(false) {
            orig.create_message(builder).await?
        } else {
            orig.callback(builder).await?
        };

        let response = response_raw
            .model()
            .await
            .wrap_err("Failed to deserialize response")?;

        let channel = response.channel_id;
        let msg = response.id;
        let (activity_tx, activity_rx) = watch::channel(());

        if let Some(until_timeout) = active_msg.until_timeout() {
            Self::spawn_timeout(activity_rx, msg, channel, until_timeout);

            let full = FullActiveMessage {
                active_msg,
                activity_tx,
            };

            Context::get().active_msgs.insert(msg, full).await;
        }

        Ok(())
    }

    pub fn attachment(self, attachment: Option<(String, Vec<u8>)>) -> Self {
        Self { attachment, ..self }
    }

    pub fn start_by_update(self, start_by_update: bool) -> Self {
        Self {
            start_by_update: Some(start_by_update),
            ..self
        }
    }

    fn spawn_timeout(
        mut rx: Receiver<()>,
        msg: Id<MessageMarker>,
        channel: Id<ChannelMarker>,
        until_timeout: Duration,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = rx.changed() => if res.is_ok() {
                        continue
                    } else {
                        return
                    },
                    _ = sleep(until_timeout) => {
                        let active_msg = Context::get().active_msgs.remove_full(msg).await;

                        if let Some(FullActiveMessage { mut active_msg, .. }) = active_msg {
                            if let Err(err) = active_msg.on_timeout(msg, channel).await {
                                warn!(?err, "Failed to timeout active message");
                            }
                        }

                        return;
                    },
                }
            }
        });
    }
}
