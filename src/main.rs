// #![allow(unused_imports, dead_code)]
mod core;
mod database;
mod util;

use crate::{
    core::{
        cache::Cache, generate_activity, handle_event, logging, BotConfig, BotStats,
        ColdRebootData, Context,
    },
    database::Database,
    util::Error,
};

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
    model::{
        gateway::{
            payload::update_status::UpdateStatusInfo,
            presence::{ActivityType, Status},
            GatewayIntents,
        },
        user::CurrentUser,
    },
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
    let user = http.current_user().await?;
    info!(
        "Token validated, connecting to Discord as {}#{}",
        user.name, user.discriminator
    );

    // Connect to the database
    let database = Database::new(&config.database.mysql).await?;
    info!("Connected to postgres database");

    // Connect to redis cache
    let redis = ConnectionPool::create(config.database.redis.clone(), None, 5).await?;
    info!("Connected to redis");

    // Boot everything up
    run(config, http, user, database, redis).await
}

async fn run(
    config: BotConfig,
    http: HttpClient,
    user: CurrentUser,
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
    let s = stats.clone();
    tokio::spawn(async move {
        let hello = warp::path!("metrics").map(move || {
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = s.registry.gather();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        });
        warp::serve(hello).run(([127, 0, 0, 1], 9091)).await;
    });
    let cache = Cache::new(stats.clone());
    let mut cb = ClusterConfig::builder(&config.tokens.discord)
        .shard_scheme(sharding_scheme)
        .intents(intents);
    // Check for resume data, pass to builder if present
    let mut connection = redis.get().await;
    match connection.get("cb_cluster_data_0").await.ok().flatten() {
        Some(d) => {
            let cold_cache: ColdRebootData = serde_json::from_str(&*String::from_utf8(d).unwrap())?;
            debug!("ColdRebootData: {:?}", cold_cache);
            connection.del("cb_cluster_data_0").await?;
            if cold_cache.total_shards == total_shards
                && cold_cache.shard_count == shards_per_cluster
            {
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
                        let end = std::time::Instant::now();
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
        }
        None => {}
    };
    let cluster_config = cb.build();
    let cluster = Cluster::new(cluster_config).await?;
    let context = Arc::new(
        Context::new(
            cache,
            cluster,
            http,
            user,
            database,
            redis.clone(),
            stats.clone(),
            total_shards,
            shards_per_cluster,
        )
        .await,
    );
    let shutdown_ctx = context.clone();
    ctrlc::set_handler(move || {
        // We need a seperate runtime, because at this point in the program,
        // the tokio::main instance isn't running anymore
        let _ = Runtime::new()
            .unwrap()
            .block_on(shutdown_ctx.initiate_cold_resume());
        process::exit(0);
    })
    .map_err(|why| format_err!("Failed to register shutdown handler: {}", why))?;
    info!("Cluster going online");
    let c = context.cluster.clone();
    tokio::spawn(async move {
        tokio::time::delay_for(Duration::from_secs(1)).await;
        c.up().await;
    });
    let mut bot_events = context.cluster.events().await;
    while let Some(event) = bot_events.next().await {
        let c = context.clone();
        let (shard, event) = event;
        context.update_stats(shard, &event);
        context.cache.update(shard, &event, context.clone()).await?;
        tokio::spawn(async move {
            if let Err(why) = handle_event(shard, &event, c).await {
                error!("Error while handling event: {}", why);
            }
        });
    }
    context.cluster.down().await;
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
