#![allow(dead_code)]

mod commands;
mod core;
mod database;
mod util;

use crate::{
    core::{
        handle_event, logging, BotConfig, BotStats, Cache, ColdRebootData, CommandGroups, Context,
    },
    database::Database,
    util::Error,
};

#[macro_use]
extern crate proc_macros;
#[macro_use]
extern crate log;

use clap::{App, Arg};
use darkredis::ConnectionPool;
use prometheus::{Encoder, TextEncoder};
use std::{
    collections::HashMap,
    process,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, stream::StreamExt};
use twilight::{
    gateway::{cluster::config::ShardScheme, shard::ResumeSession, Cluster, ClusterConfig},
    http::{
        request::channel::message::allowed_mentions::AllowedMentionsBuilder, Client as HttpClient,
    },
    model::{gateway::GatewayIntents, user::CurrentUser},
};
use warp::Filter;

pub type BotResult<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() -> BotResult<()> {
    logging::initialize()?;

    // Load config file
    let config = BotConfig::new("config.toml")?;
    info!("Loaded config file");

    //Connect to the discord http client
    let mut builder = HttpClient::builder();
    builder
        .token(&config.tokens.discord)
        .default_allowed_mentions(
            AllowedMentionsBuilder::new()
                .parse_users()
                .parse_roles()
                .build_solo(),
        );
    let http = builder.build()?;
    let bot_user = http.current_user().await?;
    info!(
        "Token validated, connecting to Discord as {}#{}",
        bot_user.name, bot_user.discriminator
    );

    // Connect to the database
    let database = Database::new(&config.database.postgres).await?;
    info!("Connected to postgres database");

    // Connect to redis cache
    let redis = ConnectionPool::create(config.database.redis.clone(), None, 5).await?;
    info!("Connected to redis");

    // Boot everything up
    run(config, http, bot_user, database, redis).await
}

async fn run(
    config: BotConfig,
    http: HttpClient,
    bot_user: CurrentUser,
    database: Database,
    redis: ConnectionPool,
) -> BotResult<()> {
    let (shards_per_cluster, total_shards, sharding_scheme) = shard_schema_values()
        .map_or((1, 1, ShardScheme::Auto), |(to, total)| {
            (to, total, ShardScheme::Range { from: 0, to, total })
        });
    let intents = Some(
        GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MEMBERS
            | GatewayIntents::GUILD_PRESENCES
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::GUILD_MESSAGE_REACTIONS
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::DIRECT_MESSAGE_REACTIONS,
    );
    let stats = Arc::new(BotStats::new());

    // // Provide stats to locale address
    // let s = stats.clone();
    // tokio::spawn(async move {
    //     let hello = warp::path!("metrics").map(move || {
    //         let mut buffer = vec![];
    //         let encoder = TextEncoder::new();
    //         let metric_families = s.registry.gather();
    //         encoder.encode(&metric_families, &mut buffer).unwrap();
    //         String::from_utf8(buffer).unwrap()
    //     });
    //     warp::serve(hello).run(([127, 0, 0, 1], 9091)).await;
    // });

    // Prepare cluster builder
    let cache = Cache::new(bot_user, stats.clone());
    let mut cb = ClusterConfig::builder(&config.tokens.discord)
        .shard_scheme(sharding_scheme)
        .intents(intents);

    // Check for resume data, pass to builder if present
    let mut connection = redis.get().await;
    let x = connection.get("cb_cluster_data_0").await;
    println!("redis result: {:?}", x);
    if let Some(d) = connection.get("cb_cluster_data_0").await.ok().flatten() {
        let cold_cache: ColdRebootData = serde_json::from_str(&*String::from_utf8(d).unwrap())?;
        debug!("ColdRebootData: {:?}", cold_cache);
        connection.del("cb_cluster_data_0").await?;
        if cold_cache.total_shards == total_shards && cold_cache.shard_count == shards_per_cluster {
            let mut map = HashMap::new();
            for (id, data) in cold_cache.resume_data {
                map.insert(
                    id,
                    ResumeSession {
                        session_id: data.0,
                        sequence: data.1,
                    },
                );
            }
            let start = Instant::now();
            let result = cache
                .restore_cold_resume(&redis, cold_cache.guild_chunks, cold_cache.user_chunks)
                .await;
            match result {
                Ok(_) => {
                    let end = Instant::now();
                    info!(
                        "Cold resume defrosting completed in {}ms",
                        (end - start).as_millis()
                    );
                    cb = cb.resume_sessions(map);
                }
                Err(why) => {
                    error!("Cold resume defrosting failed: {}", why);
                    cache.reset();
                }
            }
        }
    } else {
        println!("not found");
    }

    // Build cluster and create context
    let cluster_config = cb.build();
    let cluster = Cluster::new(cluster_config).await?;
    let ctx = Arc::new(
        Context::new(
            cache,
            cluster,
            http,
            database,
            redis.clone(),
            stats,
            total_shards,
            shards_per_cluster,
        )
        .await,
    );

    // Setup graceful shutdown
    let shutdown_ctx = ctx.clone();
    ctrlc::set_handler(move || {
        // tokio::main no longer running, create own runtime
        let freeze_result = Runtime::new()
            .unwrap()
            .block_on(shutdown_ctx.initiate_cold_resume());
        println!("freeze_result: {:?}", freeze_result);
        process::exit(0);
    })
    .map_err(|why| format_err!("Failed to register shutdown handler: {}", why))?;

    ctx.backend.cluster.up().await;
    let mut bot_events = ctx.backend.cluster.events().await;
    let cmd_groups = Arc::new(CommandGroups::new());
    while let Some(event) = bot_events.next().await {
        let c = ctx.clone();
        let (shard, event) = event;
        ctx.update_stats(shard, &event);
        ctx.cache.update(shard, &event, ctx.clone()).await?;
        let cmds = cmd_groups.clone();
        tokio::spawn(async move {
            if let Err(why) = handle_event(shard, &event, c, cmds).await {
                error!("Error while handling event: {}", why);
            }
        });
    }
    // ctx.cluster.down().await;
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
        .or(args.value_of("total_shards"))?;
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
