#![allow(clippy::upper_case_acronyms)]

macro_rules! unwind_error {
    ($log:ident, $err:ident, $($arg:tt)+) => {
        {
            $log!($($arg)+, $err);
            let mut err: &dyn ::std::error::Error = &$err;

            while let Some(source) = err.source() {
                $log!("  - caused by: {}", source);
                err = source;
            }
        }
    };
}

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
    core::{handle_event, logging, BotStats, Cache, Context, MatchLiveChannels, CONFIG},
    custom_client::CustomClient,
    database::Database,
    tracking::OsuTracking,
    twitch::Twitch,
    util::error::Error,
};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate proc_macros;

#[macro_use]
extern crate smallvec;

use dashmap::{DashMap, DashSet};
use deadpool_redis::{Config as RedisConfig, PoolConfig as RedisPoolConfig};
use hashbrown::HashSet;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Response,
};
use prometheus::{Encoder, TextEncoder};
use rosu_v2::Osu;
use smallstr::SmallString;
use std::{
    convert::Infallible,
    env, process,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    runtime::Runtime,
    signal,
    sync::{oneshot, Mutex},
    time,
};
use tokio_stream::StreamExt;
use twilight_gateway::{cluster::ShardScheme, Cluster};
use twilight_http::Client as HttpClient;
use twilight_model::{
    channel::message::allowed_mentions::AllowedMentionsBuilder,
    gateway::{
        presence::{ActivityType, Status},
        Intents,
    },
};

type CountryCode = SmallString<[u8; 2]>;
type Name = SmallString<[u8; 15]>;
type BotResult<T> = std::result::Result<T, Error>;

fn main() {
    let runtime = Runtime::new().expect("Could not start runtime");

    if let Err(why) = runtime.block_on(async move { async_main().await }) {
        unwind_error!(error, why, "Critical error in main: {}");
    }
}

async fn async_main() -> BotResult<()> {
    logging::initialize()?;
    dotenv::dotenv().expect("failed to load .env");

    // Load config file
    core::BotConfig::init("config.toml").await?;

    let config = CONFIG.get().unwrap();

    // Prepare twitch client
    let twitch = Twitch::new(&config.tokens.twitch_client_id, &config.tokens.twitch_token).await?;

    // Connect to the discord http client
    let http = HttpClient::builder()
        .token(config.tokens.discord.to_owned())
        .default_allowed_mentions(
            AllowedMentionsBuilder::new()
                .replied_user()
                .roles()
                .users()
                .build(),
        )
        .build();

    let bot_user = http.current_user().exec().await?.model().await?;

    info!(
        "Connecting to Discord as {}#{}...",
        bot_user.name, bot_user.discriminator
    );

    // Connect to psql database
    let db_uri = env::var("DATABASE_URL").expect("missing DATABASE_URL in .env");
    let psql = Database::new(&db_uri)?;

    // Connect to redis
    let redis_uri = env::var("REDIS_URL").expect("missing REDIS_URL in .env");

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

    let clients = crate::core::Clients {
        psql,
        redis,
        osu,
        custom,
        twitch,
    };

    // Boot everything up
    run(http, clients).await
}

async fn run(http: HttpClient, clients: crate::core::Clients) -> BotResult<()> {
    // Guild configs
    let guilds = clients.psql.get_guilds().await?;

    // Tracked streams
    let tracked_streams = clients.psql.get_stream_tracks().await?;

    // Reaction-role-assign
    let role_assigns = clients.psql.get_role_assigns().await?;

    // Discord-osu! links
    let discord_links = clients.psql.get_discord_links().await?;

    // osu! top score tracking
    let osu_tracking = OsuTracking::new(&clients.psql).await?;

    // snipe countries
    let snipe_countries = clients.psql.get_snipe_countries().await?;

    let data = crate::core::ContextData {
        guilds,
        tracked_streams,
        role_assigns,
        discord_links,
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

    // Prepare cluster builder
    let mut cb = Cluster::builder(&CONFIG.get().unwrap().tokens.discord, intents)
        .shard_scheme(ShardScheme::Auto);

    // Check for resume data, pass to builder if present
    let (cache, resume_map) = Cache::new(&clients.redis).await;
    let resumed = if let Some(map) = resume_map {
        cb = cb.resume_sessions(map);
        info!("Cold resume successful");

        true
    } else {
        info!("Boot without cold resume");

        false
    };

    let stats = Arc::new(BotStats::new(clients.osu.metrics(), cache.metrics()));

    // Provide stats to locale address
    let (tx, rx) = oneshot::channel();
    let metrics_stats = Arc::clone(&stats);
    tokio::spawn(_run_metrics_server(metrics_stats, rx));

    // Build cluster
    let (cluster, mut event_stream) = cb
        .build()
        .await
        .map_err(|why| format_err!("Could not start cluster: {}", why))?;

    // Final context
    let ctx = Arc::new(Context::new(cache, stats, http, clients, cluster, data).await);

    // Setup graceful shutdown
    let shutdown_ctx = Arc::clone(&ctx);

    tokio::spawn(async move {
        if let Err(err) = signal::ctrl_c().await {
            unwind_error!(error, err, "Error while waiting for ctrlc: {}");

            return;
        }

        info!("Received Ctrl+C");

        if tx.send(()).is_err() {
            error!("Failed to send shutdown message to metric loop");
        }

        // Disable tracking while preparing shutdown
        shutdown_ctx
            .tracking()
            .stop_tracking
            .store(true, Ordering::SeqCst);

        shutdown_ctx.initiate_cold_resume().await;

        if let Err(why) = shutdown_ctx.store_configs().await {
            error!("Error while storing configs: {}", why);
        }

        let count = shutdown_ctx.garbage_collect_all_maps().await;
        info!("Garbage collected {} maps", count);

        let count = shutdown_ctx.stop_all_games().await;
        info!("Stopped {} bg games", count);

        let count = shutdown_ctx.notify_match_live_shutdown().await;
        info!("Stopped match tracking in {} channels", count);

        info!("Shutting down");
        process::exit(0);
    });

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

            if let Err(why) = activity_result {
                unwind_error!(warn, why, "Error while setting activity: {}");
            }
        }
    });

    while let Some((shard, event)) = event_stream.next().await {
        ctx.update_stats(shard, &event);
        ctx.cache.update(&event);
        ctx.standby.process(&event);
        let c = Arc::clone(&ctx);

        tokio::spawn(async move {
            if let Err(why) = handle_event(shard, event, c).await {
                unwind_error!(error, why, "Error while handling event: {}");
            }
        });
    }

    info!("Exited event loop");

    // Give the ctrlc handler time to finish
    time::sleep(Duration::from_secs(300)).await;

    Ok(())
}

async fn _run_metrics_server(stats: Arc<BotStats>, shutdown_rx: oneshot::Receiver<()>) {
    let metric_service = make_service_fn(move |_| {
        let stats = Arc::clone(&stats);

        async move {
            Ok::<_, Infallible>(service_fn(move |_req| {
                let mut buffer = Vec::new();
                let encoder = TextEncoder::new();
                let metric_families = stats.registry.gather();
                encoder.encode(&metric_families, &mut buffer).unwrap();

                async move { Ok::<_, Infallible>(Response::new(Body::from(buffer))) }
            }))
        }
    });

    let ip = CONFIG.get().unwrap().metric_server_ip;
    let port = CONFIG.get().unwrap().metric_server_port;
    let addr = std::net::SocketAddr::from((ip, port));

    let server = hyper::Server::bind(&addr)
        .serve(metric_service)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

    info!("Running metrics server...");

    if let Err(why) = server.await {
        error!("Metrics server failed: {}", why);
    }
}
