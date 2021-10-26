#![deny(clippy::all, nonstandard_style, rust_2018_idioms, unused, warnings)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate proc_macros;

#[macro_use]
extern crate smallvec;

#[macro_use]
mod error;

mod arguments;
mod bg_game;
mod commands;
mod core;
mod custom_client;
mod database;
mod embeds;
mod pagination;
mod pp;
mod tracking;
mod twitch;
mod util;

use crate::{
    arguments::Args,
    commands::SLASH_COMMANDS,
    core::{
        commands::{self as cmds, CommandData, CommandDataCompact},
        logging, BotStats, Cache, Context, MatchLiveChannels, CONFIG,
    },
    custom_client::CustomClient,
    database::Database,
    error::Error,
    tracking::OsuTracking,
    twitch::Twitch,
    util::{constants::BATHBOT_WORKSHOP_ID, MessageBuilder},
};

use dashmap::{DashMap, DashSet};
use deadpool_redis::{Config as RedisConfig, PoolConfig as RedisPoolConfig};
use eyre::{Result, WrapErr};
use hashbrown::HashSet;
use parking_lot::Mutex;
use rosu_v2::Osu;
use smallstr::SmallString;
use std::{
    env,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    signal,
    sync::{mpsc, oneshot},
    time::{self, MissedTickBehavior},
};
use tokio_stream::StreamExt;
use twilight_gateway::{cluster::ShardScheme, Cluster, Event, EventTypeFlags};
use twilight_http::Client as HttpClient;
use twilight_model::{
    application::interaction::Interaction,
    channel::message::allowed_mentions::AllowedMentionsBuilder,
    gateway::{
        payload::RequestGuildMembers,
        presence::{ActivityType, Status},
        Intents,
    },
};

type Name = SmallString<[u8; 15]>;
type BotResult<T> = std::result::Result<T, Error>;

fn main() {
    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()
        .expect("Could not build runtime");

    if let Err(report) = runtime.block_on(async_main()) {
        error!("{:?}", report.wrap_err("critical error in main"));
    }
}

async fn async_main() -> Result<()> {
    logging::initialize()?;
    dotenv::dotenv()?;

    // Load config file
    core::BotConfig::init("config.toml").await?;

    let config = CONFIG.get().unwrap();

    // Prepare twitch client
    let twitch = Twitch::new(&config.tokens.twitch_client_id, &config.tokens.twitch_token).await?;

    // Connect to the discord http client
    let http = HttpClient::builder()
        .token(config.tokens.discord.to_owned())
        .remember_invalid_token(false)
        .default_allowed_mentions(
            AllowedMentionsBuilder::new()
                .replied_user()
                .roles()
                .users()
                .build(),
        )
        .build();

    let current_user = http.current_user().exec().await?.model().await?;
    let application_id = current_user.id.0.into();

    info!(
        "Connecting to Discord as {}#{}...",
        current_user.name, current_user.discriminator
    );

    http.set_application_id(application_id);

    // Connect to psql database
    let db_uri = env::var("DATABASE_URL").wrap_err("missing DATABASE_URL in .env")?;
    let psql = Database::new(&db_uri)?;

    // Connect to redis
    let redis_uri = env::var("REDIS_URL").wrap_err("missing REDIS_URL in .env")?;

    let redis_config = RedisConfig {
        connection: None,
        pool: Some(RedisPoolConfig::new(4)),
        url: Some(redis_uri),
    };

    let redis = redis_config.create_pool()?;

    // Connect to osu! API
    let osu_client_id = config.tokens.osu_client_id;
    let osu_client_secret = &config.tokens.osu_client_secret;

    let osu = Osu::new(osu_client_id, osu_client_secret).await?;

    // Log custom client into osu!
    let custom = CustomClient::new().await?;

    // Guild configs
    let guilds = psql.get_guilds().await?;

    // Tracked streams
    let tracked_streams = psql.get_stream_tracks().await?;

    // Reaction-role-assign
    let role_assigns = psql.get_role_assigns().await?;

    // osu! top score tracking
    let osu_tracking = OsuTracking::new(&psql).await?;

    // snipe countries
    let snipe_countries = psql.get_snipe_countries().await?;

    let data = crate::core::ContextData {
        guilds,
        tracked_streams,
        role_assigns,
        bg_games: DashMap::new(),
        osu_tracking,
        msgs_to_process: DashSet::new(),
        map_garbage_collection: Mutex::new(HashSet::new()),
        match_live: MatchLiveChannels::new(),
        snipe_countries,
    };

    let intents = Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MESSAGE_REACTIONS
        | Intents::DIRECT_MESSAGES
        | Intents::DIRECT_MESSAGE_REACTIONS;

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

    // Prepare cluster builder
    let mut cb = Cluster::builder(&CONFIG.get().unwrap().tokens.discord, intents)
        .event_types(EventTypeFlags::all() - ignore_flags)
        .http_client(http.clone())
        .shard_scheme(ShardScheme::Auto);

    // Check for resume data, pass to builder if present
    let (cache, resume_map) = Cache::new(&redis).await;
    let resumed = if let Some(map) = resume_map {
        cb = cb.resume_sessions(map);
        info!("Cold resume successful");

        true
    } else {
        info!("Boot without cold resume");

        false
    };

    let stats = Arc::new(BotStats::new(osu.metrics(), cache.metrics()));

    // Build cluster
    let (cluster, mut event_stream) = cb.build().await?;

    // Slash commands
    let slash_commands = SLASH_COMMANDS.collect();
    info!("Setting {} slash commands...", slash_commands.len());

    if cfg!(debug_assertions) {
        http.set_guild_commands(BATHBOT_WORKSHOP_ID.into(), &slash_commands)?
            .exec()
            .await?;
    } else {
        http.set_global_commands(&slash_commands)?.exec().await?;
    }

    let clients = crate::core::Clients {
        psql,
        redis,
        osu,
        custom,
        twitch,
    };

    let (member_tx, mut member_rx) = mpsc::unbounded_channel();

    // Final context
    let ctx = Arc::new(Context::new(cache, stats, http, clients, cluster, data, member_tx).await);

    // Spawn server worker
    let server_ctx = Arc::clone(&ctx);
    let (tx, rx) = oneshot::channel();
    tokio::spawn(core::server::run_server(server_ctx, rx));

    // Spawn twitch worker
    let twitch_ctx = Arc::clone(&ctx);
    tokio::spawn(twitch::twitch_loop(twitch_ctx));

    // Spawn osu tracking worker
    let osu_tracking_ctx = Arc::clone(&ctx);
    tokio::spawn(tracking::tracking_loop(osu_tracking_ctx));

    // Spawn background loop worker
    let background_ctx = Arc::clone(&ctx);
    tokio::spawn(Context::background_loop(background_ctx));

    // Spawn osu match ticker worker
    let match_live_ctx = Arc::clone(&ctx);
    tokio::spawn(Context::match_live_loop(match_live_ctx));

    // Activate cluster
    let cluster_ctx = Arc::clone(&ctx);

    tokio::spawn(async move {
        time::sleep(Duration::from_secs(1)).await;
        cluster_ctx.cluster.up().await;

        if resumed {
            time::sleep(Duration::from_secs(5)).await;

            let activity_result = cluster_ctx
                .set_cluster_activity(Status::Online, ActivityType::Playing, "osu!")
                .await;

            if let Err(report) = activity_result.wrap_err("error while setting activity") {
                warn!("{:?}", report);
            }
        }
    });

    // Request members
    let member_ctx = Arc::clone(&ctx);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(100));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await;
        info!("Start requesting members...");

        while let Some((guild_id, shard_id)) = member_rx.recv().await {
            interval.tick().await;
            let req = RequestGuildMembers::builder(guild_id).query("", None);

            let command_result = member_ctx
                .cluster
                .command(shard_id, &req)
                .await
                .wrap_err_with(|| format!("failed to request members for guild {}", guild_id));

            if let Err(report) = command_result {
                warn!("{:?}", report);
            }
        }
    });

    let event_loop = async {
        while let Some((shard_id, event)) = event_stream.next().await {
            ctx.cache.update(&event);
            ctx.standby.process(&event);
            let ctx = Arc::clone(&ctx);

            tokio::spawn(async move {
                let handle_fut = handle_event(ctx, event, shard_id);

                if let Err(report) = handle_fut.await.wrap_err("error while handling event") {
                    error!("{:?}", report);
                }
            });
        }
    };

    tokio::select! {
        _ = event_loop => error!("Event loop ended"),
        res = signal::ctrl_c() => if let Err(report) = res.wrap_err("error while awaiting ctrl+c") {
            error!("{:?}", report);
        } else {
            info!("Received Ctrl+C");
        },
    }

    if tx.send(()).is_err() {
        error!("Failed to send shutdown message to server");
    }

    // Disable tracking while preparing shutdown
    ctx.tracking().stop_tracking.store(true, Ordering::SeqCst);

    // Prevent non-minimized msgs from getting minimized
    ctx.clear_msgs_to_process();

    ctx.initiate_cold_resume().await;

    let (count, total) = ctx.garbage_collect_all_maps().await;
    info!("Garbage collected {}/{} maps", count, total);

    let count = ctx.stop_all_games().await;
    info!("Stopped {} bg games", count);

    let count = ctx.notify_match_live_shutdown().await;
    info!("Stopped match tracking in {} channels", count);

    info!("Shutting down");

    Ok(())
}

async fn handle_event(ctx: Arc<Context>, event: Event, shard_id: u64) -> BotResult<()> {
    match event {
        Event::BanAdd(_) => {}
        Event::BanRemove(_) => {}
        Event::ChannelCreate(_) => ctx.stats.event_counts.channel_create.inc(),
        Event::ChannelDelete(_) => ctx.stats.event_counts.channel_delete.inc(),
        Event::ChannelPinsUpdate(_) => {}
        Event::ChannelUpdate(_) => ctx.stats.event_counts.channel_update.inc(),
        Event::GatewayHeartbeat(_) => {}
        Event::GatewayHeartbeatAck => {}
        Event::GatewayHello(_) => {}
        Event::GatewayInvalidateSession(reconnect) => {
            ctx.stats.event_counts.gateway_invalidate.inc();

            if reconnect {
                warn!(
                    "Gateway has invalidated session for shard {}, but its reconnectable",
                    shard_id
                );
            } else {
                warn!("Gateway has invalidated session for shard {}", shard_id);
            }
        }
        Event::GatewayReconnect => {
            info!("Gateway requested shard {} to reconnect", shard_id);
            ctx.stats.event_counts.gateway_reconnect.inc();
        }
        Event::GiftCodeUpdate => {}
        Event::GuildCreate(_) => {
            ctx.stats.event_counts.guild_create.inc();

            // if let Err(why) = ctx.member_tx.send((e.id, shard_id)) {
            //     warn!("Failed to forward member request: {}", why);
            // }
        }
        Event::GuildDelete(_) => ctx.stats.event_counts.guild_delete.inc(),
        Event::GuildEmojisUpdate(_) => {}
        Event::GuildIntegrationsUpdate(_) => {}
        Event::GuildUpdate(_) => ctx.stats.event_counts.guild_update.inc(),
        Event::IntegrationCreate(_) => {}
        Event::IntegrationDelete(_) => {}
        Event::IntegrationUpdate(_) => {}
        Event::InteractionCreate(e) => {
            ctx.stats.event_counts.interaction_create.inc();

            match e.0 {
                Interaction::ApplicationCommand(cmd) => {
                    cmds::handle_command(ctx, *cmd).await?;
                }
                Interaction::MessageComponent(component) => {
                    cmds::handle_component(ctx, component).await?;
                }
                _ => {}
            }
        }
        Event::InviteCreate(_) => {}
        Event::InviteDelete(_) => {}
        Event::MemberAdd(_) => ctx.stats.event_counts.member_add.inc(),
        Event::MemberRemove(_) => ctx.stats.event_counts.member_remove.inc(),
        Event::MemberUpdate(_) => ctx.stats.event_counts.member_update.inc(),
        Event::MemberChunk(_) => ctx.stats.event_counts.member_chunk.inc(),
        Event::MessageCreate(msg) => {
            ctx.stats.event_counts.message_create.inc();

            if !msg.author.bot {
                ctx.stats.message_counts.user_messages.inc()
            } else if ctx.is_own(&*msg) {
                ctx.stats.message_counts.own_messages.inc()
            } else {
                ctx.stats.message_counts.other_bot_messages.inc()
            }

            cmds::handle_message(ctx, msg.0).await?;
        }
        Event::MessageDelete(msg) => {
            ctx.stats.event_counts.message_delete.inc();
            ctx.remove_msg(msg.id);
        }
        Event::MessageDeleteBulk(msgs) => {
            ctx.stats.event_counts.message_delete_bulk.inc();

            for id in msgs.ids.into_iter() {
                ctx.remove_msg(id);
            }
        }
        Event::MessageUpdate(_) => ctx.stats.event_counts.message_update.inc(),
        Event::PresenceUpdate(_) => {}
        Event::PresencesReplace => {}
        Event::ReactionAdd(reaction_add) => {
            ctx.stats.event_counts.reaction_add.inc();
            let reaction = &reaction_add.0;

            if let Some(guild_id) = reaction.guild_id {
                if let Some(role_id) = ctx.get_role_assign(reaction) {
                    let add_role_fut =
                        ctx.http
                            .add_guild_member_role(guild_id, reaction.user_id, role_id);

                    match add_role_fut.exec().await {
                        Ok(_) => debug!("Assigned react-role to user"),
                        Err(why) => error!("Error while assigning react-role to user: {}", why),
                    }
                }
            }
        }
        Event::ReactionRemove(reaction_remove) => {
            ctx.stats.event_counts.reaction_remove.inc();
            let reaction = &reaction_remove.0;

            if let Some(guild_id) = reaction.guild_id {
                if let Some(role_id) = ctx.get_role_assign(reaction) {
                    let remove_role_fut =
                        ctx.http
                            .remove_guild_member_role(guild_id, reaction.user_id, role_id);

                    match remove_role_fut.exec().await {
                        Ok(_) => debug!("Removed react-role from user"),
                        Err(why) => error!("Error while removing react-role from user: {}", why),
                    }
                }
            }
        }
        Event::ReactionRemoveAll(_) => ctx.stats.event_counts.reaction_remove_all.inc(),
        Event::ReactionRemoveEmoji(_) => ctx.stats.event_counts.reaction_remove_emoji.inc(),
        Event::Ready(_) => {
            info!("Shard {} is ready", shard_id);

            let result = ctx
                .set_shard_activity(shard_id, Status::Online, ActivityType::Playing, "osu!")
                .await
                .wrap_err_with(|| format!("failed to set activity for shard {}", shard_id));

            if let Err(report) = result {
                error!("{:?}", report);
            } else {
                info!("Game is set for shard {}", shard_id);
            }
        }
        Event::Resumed => info!("Shard {} is resumed", shard_id),
        Event::RoleCreate(_) => ctx.stats.event_counts.role_create.inc(),
        Event::RoleDelete(_) => ctx.stats.event_counts.role_delete.inc(),
        Event::RoleUpdate(_) => ctx.stats.event_counts.role_update.inc(),
        Event::ShardConnected(_) => info!("Shard {} is connected", shard_id),
        Event::ShardConnecting(_) => info!("Shard {} is connecting...", shard_id),
        Event::ShardDisconnected(_) => info!("Shard {} is disconnected", shard_id),
        Event::ShardIdentifying(_) => info!("Shard {} is identifying...", shard_id),
        Event::ShardReconnecting(_) => info!("Shard {} is reconnecting...", shard_id),
        Event::ShardPayload(_) => {}
        Event::ShardResuming(_) => info!("Shard {} is resuming...", shard_id),
        Event::StageInstanceCreate(_) => {}
        Event::StageInstanceDelete(_) => {}
        Event::StageInstanceUpdate(_) => {}
        Event::ThreadCreate(_) => {}
        Event::ThreadDelete(_) => {}
        Event::ThreadListSync(_) => {}
        Event::ThreadMemberUpdate(_) => {}
        Event::ThreadMembersUpdate(_) => {}
        Event::ThreadUpdate(_) => {}
        Event::TypingStart(_) => {}
        Event::UnavailableGuild(_) => ctx.stats.event_counts.unavailable_guild.inc(),
        Event::UserUpdate(_) => ctx.stats.event_counts.user_update.inc(),
        Event::VoiceServerUpdate(_) => {}
        Event::VoiceStateUpdate(_) => {}
        Event::WebhooksUpdate(_) => {}
    }

    Ok(())
}
