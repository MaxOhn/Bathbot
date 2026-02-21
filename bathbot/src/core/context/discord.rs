use std::{collections::HashMap, sync::Arc, time::Duration};

use bathbot_util::IntHasher;
use eyre::{Report, Result, WrapErr};
use tokio::{
    sync::{Mutex as TokioMutex, broadcast, mpsc::UnboundedReceiver},
    time::{self, MissedTickBehavior},
};
use twilight_gateway::{CloseFrame, ConfigBuilder, Intents, Session, Shard, ShardId};
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
) -> Result<impl ExactSizeIterator<Item = Shard>> {
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

    twilight_gateway::create_recommended(http, config, config_callback)
        .await
        .wrap_err("Failed to create recommended shards")
}

impl Context {
    pub fn down_resumable(shards: &[Shard]) -> HashMap<u32, Session, IntHasher> {
        shards
            .iter()
            .filter_map(|shard| {
                shard.close(CloseFrame::RESUME);

                shard
                    .session()
                    .map(|session| (shard.id().number(), session.clone()))
            })
            .collect()
    }

    pub async fn reshard_loop(sender: broadcast::Sender<()>) {
        const HALF_DAY: Duration = Duration::from_hours(12);

        let mut interval = time::interval(HALF_DAY);
        interval.tick().await;

        loop {
            interval.tick().await;

            if sender.send(()).is_ok() {
                info!("Autosharding...");
            } else {
                error!("Reshard receiver has been dropped");
            }
        }
    }

    pub async fn reshard(shards: &mut Vec<Arc<TokioMutex<Shard>>>) -> Result<()> {
        info!("Resharding...");

        {
            // Tell the current shards to close
            let unlocked = Context::get().shard_senders.read().unwrap();

            for sender in unlocked.values() {
                let _: Result<_, _> = sender.close(CloseFrame::RESUME);
            }
        }

        // Creating new shards
        let shards_iter = gateway(BotConfig::get(), Context::http(), HashMap::default())
            .await
            .wrap_err("Failed to create new shards for resharding")?;

        shards.clear();
        let mut senders = HashMap::default();

        for shard in shards_iter {
            senders.insert(shard.id().number(), shard.sender());
            shards.push(Arc::new(TokioMutex::new(shard)));
        }

        // Storing shard senders
        *Context::get().shard_senders.write().unwrap() = senders;

        info!("Finished resharding");

        Ok(())
    }

    pub async fn request_guild_members(mut member_rx: UnboundedReceiver<(Id<GuildMarker>, u32)>) {
        const TEN_MINUTES: Duration = Duration::from_mins(10);

        let ctx = Context::get();
        let mut interval = time::interval(TEN_MINUTES);
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
