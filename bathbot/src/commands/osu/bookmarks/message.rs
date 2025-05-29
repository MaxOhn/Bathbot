use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

use bathbot_macros::msg_command;
use bathbot_util::{Authored, MessageOrigin, constants::GENERAL_ISSUE, osu::MapIdType};
use eyre::{Report, Result};
use rosu_v2::prelude::OsuError;
use twilight_model::channel::Message;

use crate::{
    active::{ActiveMessages, impls::BookmarksPagination},
    core::Context,
    util::{InteractionCommandExt, interaction::InteractionCommand, osu::MapOrScore},
};

#[msg_command(name = "Bookmark map", flags(EPHEMERAL))]
async fn bookmark_map(mut command: InteractionCommand) -> Result<()> {
    let msg_opt = command
        .data
        .resolved
        .as_ref()
        .and_then(|resolved| resolved.messages.values().next());

    let Some(msg) = msg_opt else {
        let _ = command.error(GENERAL_ISSUE).await;

        bail!("Missing resolved message");
    };

    let map_id = match MapOrScore::find_in_msg(msg).await {
        Some(MapOrScore::Map(MapIdType::Map(map_id))) => map_id,
        Some(MapOrScore::Map(MapIdType::Set(mapset_id))) => {
            let content = format!(
                "I found the mapset id {mapset_id} in [this message]({url}) but I need a map id",
                url = MessageUrl::new(msg)
            );

            command.error(content).await?;

            return Ok(());
        }
        Some(MapOrScore::Score { .. }) => {
            let content = format!(
                "I found a score id in [this message]({url}) but I need a map id",
                url = MessageUrl::new(msg)
            );

            command.error(content).await?;

            return Ok(());
        }
        None => {
            let content = format!(
                "Could not find map in [this message]({url}).\n\
                Be sure either:\n\
                - the message content is a map url\n\
                - the embed author url is a map url\n\
                - the embed url is a map url",
                url = MessageUrl::new(msg)
            );

            command.error(content).await?;

            return Ok(());
        }
    };

    let mapset = match Context::osu().beatmapset_from_map_id(map_id).await {
        Ok(mapset) => mapset,
        Err(OsuError::NotFound) => {
            let content = format!(
                "I found the map id {map_id} in [this message]({url}) \
                but I couldn't find a map with that id",
                url = MessageUrl::new(msg)
            );

            command.error(content).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await;

            return Err(Report::new(err).wrap_err("Failed to get mapset"));
        }
    };

    let mapset_clone = mapset.clone();
    tokio::spawn(async move { Context::osu_map().store(&mapset_clone).await });

    let map_opt = mapset
        .maps
        .as_ref()
        .and_then(|maps| maps.iter().find(|map| map.map_id == map_id));

    let Some(map) = map_opt else {
        let content = format!(
            "I found the map id {map_id} in [this message]({url}) \
            but I couldn't find a map with that id in the mapset",
            url = MessageUrl::new(msg)
        );

        command.error(content).await?;

        return Ok(());
    };

    let user_id = command.user_id()?;

    let bookmarks = match Context::bookmarks().get(user_id).await {
        Ok(bookmarks) => bookmarks,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let idx = match bookmarks.iter().position(|x| x.map_id == map_id) {
        Some(index) => index,
        None => {
            if let Err(err) = Context::bookmarks().add(user_id, map_id).await {
                let _ = command.error(GENERAL_ISSUE).await;

                return Err(err);
            } else {
                0
            }
        }
    };

    debug!(user = %user_id, map = map_id, "Added bookmarked map");

    let bookmarks = match Context::bookmarks().get(user_id).await {
        Ok(bookmarks) => bookmarks,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let content = format!(
        "Successfully added [bookmarked map]({map_url}) based on [this message]({msg_url})",
        map_url = map.url,
        msg_url = MessageUrl::new(msg)
    );

    let origin = MessageOrigin::new(command.guild_id(), command.channel_id());

    let mut pagination = BookmarksPagination::builder()
        .bookmarks(bookmarks)
        .origin(origin)
        .cached_entries(HashMap::default())
        .defer_next(false)
        .token(command.token.clone())
        .content(content)
        .msg_owner(user_id)
        .build();

    pagination.set_index(idx);

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}

struct MessageUrl<'m> {
    msg: &'m Message,
}

impl<'m> MessageUrl<'m> {
    fn new(msg: &'m Message) -> Self {
        Self { msg }
    }
}

impl Display for MessageUrl<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.msg.guild_id {
            Some(guild) => write!(
                f,
                "https://discord.com/channels/{guild}/{channel}/{msg}",
                channel = self.msg.channel_id,
                msg = self.msg.id
            ),
            None => write!(
                f,
                "https://discord.com/channels/@me/{channel}/{msg}",
                channel = self.msg.channel_id,
                msg = self.msg.id
            ),
        }
    }
}
