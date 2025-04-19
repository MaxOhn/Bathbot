use std::{collections::HashMap, sync::Arc, time::Duration};

use bathbot_util::IntHasher;
use eyre::{Report, Result, WrapErr};
use tokio::{
    sync::{Mutex as TokioMutex, mpsc::UnboundedReceiver},
    time::{self, MissedTickBehavior},
};
use twilight_gateway::{ConfigBuilder, Intents, Session, Shard, ShardId};
use twilight_http::Client;
use twilight_model::{
    channel::message::AllowedMentions,
    gateway::{
        payload::outgoing::{RequestGuildMembers, update_presence::UpdatePresencePayload},
        presence::{ActivityType, MinimalActivity, Status},
    },
    id::{
        Id,
        marker::{ApplicationMarker, GuildMarker},
    },
};

use crate::core::{BotConfig, Context};

pub(super) async fn http(config: &BotConfig) -> Result<(Arc<Client>, Id<ApplicationMarker>)> {
    let mentions = AllowedMentions {
        replied_user: true,
        ..Default::default()
    };

    // Connect to the discord http client
    let http = Client::builder()
        .token(config.tokens.discord.to_string())
        .remember_invalid_token(false)
        .default_allowed_mentions(mentions)
        .build();

    let http = Arc::new(http);

    let current_user = http
        .current_user()
        .await
        .wrap_err("Failed to get current user")?
        .model()
        .await
        .wrap_err("Failed to deserialize current user")?;

    let application_id = current_user.id.cast();

    info!(
        "Connecting to Discord as {}#{:04}...",
        current_user.name, current_user.discriminator
    );

    Ok((http, application_id))
}

pub(super) async fn gateway(
    config: &BotConfig,
    http: &Client,
    resume_data: HashMap<u32, Session, IntHasher>,
) -> Result<impl ExactSizeIterator<Item = Arc<TokioMutex<Shard>>>> {
    let intents = Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MESSAGES
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT;

    let activity = MinimalActivity {
        kind: ActivityType::Playing,
        name: "osu!".to_owned(),
        url: None,
    };

    let presence =
        UpdatePresencePayload::new([activity.into()], false, None, Status::Online).unwrap();

    let config = ConfigBuilder::new(config.tokens.discord.to_string(), intents)
        .presence(presence)
        .build();

    let config_callback = move |shard_id: ShardId, builder: ConfigBuilder| match resume_data
        .get(&shard_id.number())
    {
        Some(session) => builder.session(session.to_owned()).build(),
        None => builder.build(),
    };

    let shards = twilight_gateway::create_recommended(http, config, config_callback)
        .await
        .wrap_err("Failed to create recommended shards")?;

    Ok(shards.map(TokioMutex::new).map(Arc::new))
}

impl Context {
    pub async fn request_guild_members(mut member_rx: UnboundedReceiver<(Id<GuildMarker>, u32)>) {
        let ctx = Context::get();

        let mut interval = time::interval(Duration::from_millis(600));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await;
        let mut counter = 1;
        info!("Processing member request queue...");

        while let Some((guild_id, shard_id)) = member_rx.recv().await {
            let removed_opt = ctx
                .member_requests
                .pending_guilds
                .lock()
                .unwrap()
                .remove(&guild_id);

            // If a guild is in the channel twice, only process the first and ignore the
            // second
            if !removed_opt {
                continue;
            }

            interval.tick().await;

            let req = RequestGuildMembers::builder(guild_id).query("", None);
            trace!("Member request #{counter} for guild {guild_id}");
            counter += 1;

            let command_res = match ctx.shard_senders.read().unwrap().get(&shard_id) {
                Some(sender) => sender.command(&req),
                None => {
                    warn!("Missing sender for shard {shard_id}");

                    continue;
                }
            };

            if let Err(err) = command_res {
                let wrap = format!("Failed to request members for guild {guild_id}");
                warn!("{:?}", Report::new(err).wrap_err(wrap));

                if let Err(err) = ctx.member_requests.tx.send((guild_id, shard_id)) {
                    warn!("Failed to re-forward member request: {err}");
                }
            }
        }
    }
}
