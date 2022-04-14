use std::{collections::HashMap, sync::Arc};

use twilight_gateway::{
    cluster::{Events, ShardScheme},
    shard::ResumeSession,
    Cluster, EventTypeFlags, Intents,
};
use twilight_http::Client;
use twilight_model::gateway::{
    payload::outgoing::update_presence::UpdatePresencePayload,
    presence::{ActivityType, MinimalActivity, Status},
};

use crate::BotResult;

pub async fn build_cluster(
    token: &str,
    http: Arc<Client>,
    resume_data: HashMap<u64, ResumeSession>,
) -> BotResult<(Cluster, Events)> {
    let intents = Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MESSAGE_REACTIONS
        | Intents::DIRECT_MESSAGES
        | Intents::DIRECT_MESSAGE_REACTIONS
        | Intents::MESSAGE_CONTENT;

    let ignore_flags = EventTypeFlags::BAN_ADD
        | EventTypeFlags::BAN_REMOVE
        | EventTypeFlags::CHANNEL_PINS_UPDATE
        | EventTypeFlags::GIFT_CODE_UPDATE
        | EventTypeFlags::GUILD_INTEGRATIONS_UPDATE
        | EventTypeFlags::INTEGRATION_CREATE
        | EventTypeFlags::INTEGRATION_DELETE
        | EventTypeFlags::INTEGRATION_UPDATE
        | EventTypeFlags::INVITE_CREATE
        | EventTypeFlags::INVITE_DELETE
        | EventTypeFlags::PRESENCE_UPDATE
        | EventTypeFlags::PRESENCES_REPLACE
        | EventTypeFlags::SHARD_PAYLOAD
        | EventTypeFlags::STAGE_INSTANCE_CREATE
        | EventTypeFlags::STAGE_INSTANCE_DELETE
        | EventTypeFlags::STAGE_INSTANCE_UPDATE
        | EventTypeFlags::TYPING_START
        | EventTypeFlags::VOICE_SERVER_UPDATE
        | EventTypeFlags::VOICE_STATE_UPDATE
        | EventTypeFlags::WEBHOOKS_UPDATE;

    let activity = MinimalActivity {
        kind: ActivityType::Playing,
        name: "osu!".to_owned(),
        url: None,
    };

    let presence =
        UpdatePresencePayload::new([activity.into()], false, None, Status::Online).unwrap();

    let tuple = Cluster::builder(token.to_owned(), intents)
        .event_types(EventTypeFlags::all() - ignore_flags)
        .http_client(http)
        .shard_scheme(ShardScheme::Auto)
        .resume_sessions(resume_data)
        .presence(presence)
        .build()
        .await?;

    Ok(tuple)
}
