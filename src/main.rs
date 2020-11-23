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
    core::{handle_event, logging, BotStats, Cache, CommandGroups, Context, CONFIG},
    custom_client::CustomClient,
    database::Database,
    tracking::OsuTracking,
    twitch::Twitch,
    util::error::Error,
};

#[macro_use]
extern crate proc_macros;
#[macro_use]
extern crate log;

use clap::{App, Arg};
use darkredis::ConnectionPool;
use dashmap::{DashMap, DashSet};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Response,
};
use prometheus::{Encoder, TextEncoder};
use rosu::{Osu, OsuCached};
use std::{convert::Infallible, process, str::FromStr, sync::Arc, time::Duration};
use tokio::{runtime::Runtime, signal, stream::StreamExt, sync::Mutex, time};
use twilight_gateway::{cluster::ShardScheme, Cluster};
use twilight_http::{
    request::channel::message::allowed_mentions::AllowedMentionsBuilder, Client as HttpClient,
};
use twilight_model::gateway::{
    presence::{ActivityType, Status},
    Intents,
};

pub type BotResult<T> = std::result::Result<T, Error>;

fn main() -> BotResult<()> {
    let mut runtime = Runtime::new().expect("Could not start runtime");
    if let Err(why) = runtime.block_on(async move { async_main().await }) {
        unwind_error!(error, why, "Critical error in main: {}");
    }
    Ok(())
}

async fn async_main() -> BotResult<()> {
    logging::initialize()?;

    // Load config file
    core::BotConfig::init("config.toml").await?;

    // Prepare twitch client
    let twitch = Twitch::new(
        &CONFIG.get().unwrap().tokens.twitch_client_id,
        &CONFIG.get().unwrap().tokens.twitch_token,
    )
    .await?;

    // Connect to the discord http client
    let http = HttpClient::builder()
        .token(&CONFIG.get().unwrap().tokens.discord)
        .default_allowed_mentions(
            AllowedMentionsBuilder::new()
                .parse_users()
                .parse_roles()
                .build_solo(),
        )
        .build()?;
    let bot_user = http.current_user().await?;
    info!(
        "Connecting to Discord as {}#{}...",
        bot_user.name, bot_user.discriminator
    );

    // Connect to psql database and redis cache
    let psql = Database::new(&CONFIG.get().unwrap().database.postgres).await?;
    let redis =
        ConnectionPool::create(CONFIG.get().unwrap().database.redis.clone(), None, 5).await?;

    // Connect to osu! API
    let osu_token = &CONFIG.get().unwrap().tokens.osu;
    let osu = Osu::builder(osu_token, redis.clone())
        .cache_duration(300)
        .add_cached(OsuCached::User)
        .build()?;

    // Log custom client into osu!
    let custom = CustomClient::new(&CONFIG.get().unwrap().tokens.osu_session).await?;

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

    // Stored pp and star values for mania and ctb
    let stored_values = core::StoredValues::new(&clients.psql).await?;

    // osu! top score tracking
    let osu_tracking = OsuTracking::new(&clients.psql).await?;

    let data = crate::core::ContextData {
        guilds,
        tracked_streams,
        role_assigns,
        stored_values,
        perf_calc_mutex: Mutex::new(()),
        discord_links,
        bg_games: DashMap::new(),
        osu_tracking,
        msgs_to_process: DashSet::new(),
    };

    // Shard-cluster config
    let (shards_per_cluster, total_shards, sharding_scheme) = shard_schema_values()
        .map_or((1, 1, ShardScheme::Auto), |(to, total)| {
            (to, total, ShardScheme::Range { from: 0, to, total })
        });
    let intents = Intents::GUILDS
        | Intents::GUILD_MEMBERS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MESSAGE_REACTIONS
        | Intents::DIRECT_MESSAGES
        | Intents::DIRECT_MESSAGE_REACTIONS;

    // Prepare cluster builder
    let mut cb = Cluster::builder(&CONFIG.get().unwrap().tokens.discord, intents)
        .shard_scheme(sharding_scheme);

    // Check for resume data, pass to builder if present
    let (cache, resume_map) = Cache::new(&clients.redis, total_shards, shards_per_cluster).await;
    let resumed = if let Some(map) = resume_map {
        cb = cb.resume_sessions(map);
        info!("Cold resume successfull");
        true
    } else {
        info!("Boot without cold resume");
        false
    };

    let cache_metrics = cache.stats(0, 0);
    let stats = Arc::new(BotStats::new(clients.osu.metrics(), cache_metrics.metrics));

    // Provide stats to locale address
    let metrics_stats = Arc::clone(&stats);
    tokio::spawn(_run_metrics_server(metrics_stats));

    // Build cluster
    let cluster = cb
        .build()
        .await
        .map_err(|why| format_err!("Could not start cluster: {}", why))?;

    let backend = crate::core::BackendData {
        cluster,
        total_shards,
        shards_per_cluster,
    };

    // Final context
    let ctx = Arc::new(Context::new(cache, stats, http, clients, backend, data).await);

    // Setup graceful shutdown
    let shutdown_ctx = Arc::clone(&ctx);
    tokio::spawn(async move {
        if let Err(err) = signal::ctrl_c().await {
            unwind_error!(error, err, "Error while waiting for ctrlc: {}");
            return;
        }
        info!("Received Ctrl+C");
        shutdown_ctx.initiate_cold_resume().await;
        if let Err(why) = shutdown_ctx.store_configs().await {
            error!("Error while storing configs: {}", why);
        }
        if let Err(why) = shutdown_ctx.store_values().await {
            error!("Error while storing values: {}", why);
        }
        info!("Shutting down");
        process::exit(0);
    });

    // Spawn twitch worker
    let twitch_ctx = Arc::clone(&ctx);
    tokio::spawn(twitch::twitch_loop(twitch_ctx));

    // Spawn osu tracking worker
    let osu_tracking_ctx = Arc::clone(&ctx);
    tokio::spawn(tracking::tracking_loop(osu_tracking_ctx));

    // Activate cluster
    let cluster_ctx = Arc::clone(&ctx);
    tokio::spawn(async move {
        time::delay_for(Duration::from_secs(1)).await;
        cluster_ctx.backend.cluster.up().await;
        if resumed {
            time::delay_for(Duration::from_secs(5)).await;
            let activity_result = cluster_ctx
                .set_cluster_activity(Status::Online, ActivityType::Playing, String::from("osu!"))
                .await;
            if let Err(why) = activity_result {
                unwind_error!(warn, why, "Error while setting activity: {}");
            }
        }
    });

    let mut bot_events = ctx.backend.cluster.events();
    let cmd_groups = Arc::new(CommandGroups::new());
    while let Some((shard, event)) = bot_events.next().await {
        ctx.update_stats(shard, &event);
        ctx.cache.update(&event);
        ctx.standby.process(&event);
        let c = Arc::clone(&ctx);
        let cmds = Arc::clone(&cmd_groups);
        tokio::spawn(async move {
            if let Err(why) = handle_event(shard, event, c, cmds).await {
                unwind_error!(error, why, "Error while handling event: {}");
            }
        });
    }

    info!("Exited event loop");

    // Give the ctrlc handler time to finish
    time::delay_for(time::Duration::from_secs(90)).await;
    Ok(())
}

fn shard_schema_values() -> Option<(u64, u64)> {
    // Setup CLI arguments
    let args = App::new("bathbot")
        .arg(
            Arg::with_name("total shards")
                .short("s")
                .long("shards")
                .takes_value(true)
                .help("How many shards in total"),
        )
        .arg(
            Arg::with_name("shards per cluster")
                .short("c")
                .long("per_cluster")
                .takes_value(true)
                .help("How many shards per cluster"),
        )
        .get_matches();
    // Either of them given?
    args.value_of("shards_per_cluster")
        .or_else(|| args.value_of("total_shards"))?;
    // If so, parse
    let shards_per_cluster = args
        .value_of("shards_per_cluster")
        .map(u64::from_str)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or(1);
    let total_shards = args
        .value_of("total_shards")
        .map(u64::from_str)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or(1);
    Some((shards_per_cluster, total_shards))
}

async fn _run_metrics_server(stats: Arc<BotStats>) {
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
    let server = hyper::Server::bind(&addr).serve(metric_service);
    debug!("Running metrics server...");
    if let Err(why) = server.await {
        error!("Metrics server failed: {}", why);
    }
}
