use bytes::Bytes;
use eyre::{Report, Result};
use twilight_model::channel::Attachment;

use crate::{Client, Site};

impl Client {
    pub async fn get_discord_attachment(&self, attachment: &Attachment) -> Result<Bytes> {
        self.make_get_request(&attachment.url, Site::DiscordAttachment)
            .await
            .map_err(Report::new)
    }
}
