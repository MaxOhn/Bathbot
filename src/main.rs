#![allow(dead_code)]
#![allow(unused_imports)]

// TODO: Use emotes from config

mod commands;
mod core;
mod custom_client;
mod database;
mod embeds;
mod pagination;
mod pp;
mod twitch;
mod util;

use crate::{
    core::{
        handle_event, logging, BackendData, BotConfig, BotStats, Cache, Clients, ColdRebootData,
        CommandGroups, Context,
    },
    custom_client::CustomClient,
    database::Database,
    twitch::Twitch,
    util::error::Error,
};

#[macro_use]
extern crate proc_macros;
#[macro_use]
extern crate log;

use clap::{App, Arg};
use darkredis::ConnectionPool;
use dashmap::DashMap;
use prometheus::{Encoder, TextEncoder};
use rosu::{models::GameMods, Osu};
use std::{
    collections::HashMap,
    process,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, stream::StreamExt, time};
use twilight::gateway::{
    cluster::config::ShardScheme, shard::ResumeSession, Cluster, ClusterConfig,
};
use twilight::http::{
    request::channel::message::allowed_mentions::AllowedMentionsBuilder, Client as HttpClient,
};
use twilight::model::{gateway::GatewayIntents, user::CurrentUser};
use warp::Filter;

pub type BotResult<T> = std::result::Result<T, Error>;

fn main() -> BotResult<()> {
    let mut runtime = Runtime::new().expect("Could not start runtime");
    runtime.block_on(async move { async_main().await })?;
    runtime.shutdown_timeout(Duration::from_secs(90));
    Ok(())
}

async fn async_main() -> BotResult<()> {
    logging::initialize()?;

    // Load config file
    let config = BotConfig::new("config.toml")?;

    // Connect to osu! API
    let osu = Osu::new(config.tokens.osu.clone());

    // Log custom client into osu!
    let custom = CustomClient::new(&config.tokens.osu_session).await?;

    // Prepare twitch client
    let twitch = Twitch::new(&config.tokens.twitch_client_id, &config.tokens.twitch_token).await?;

    // Connect to the discord http client
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
        "Connecting to Discord as {}#{}...",
        bot_user.name, bot_user.discriminator
    );

    // Connect to psql database and redis cache
    let psql = Database::new(&config.database.postgres).await?;
    let redis = ConnectionPool::create(config.database.redis.clone(), None, 5).await?;

    let clients = Clients {
        psql,
        redis,
        osu,
        custom,
    };

    // Boot everything up
    run(config, http, bot_user, clients, twitch).await
}

async fn run(
    config: BotConfig,
    http: HttpClient,
    bot_user: CurrentUser,
    clients: Clients,
    twitch: Twitch,
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
    let stored_values = core::StoredValues::new(&clients.psql).await?;

    // Provide stats to locale address
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
    let cache = Cache::new(bot_user, stats);
    let mut cb = ClusterConfig::builder(&config.tokens.discord)
        .shard_scheme(sharding_scheme)
        .intents(intents);

    // Check for resume data, pass to builder if present
    {
        let mut connection = clients.redis.get().await;
        if let Some(d) = connection.get("cb_cluster_data_0").await.ok().flatten() {
            let cold_cache: ColdRebootData = serde_json::from_str(&*String::from_utf8(d).unwrap())?;
            debug!("ColdRebootData:\n{:#?}", cold_cache);
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
                    .restore_cold_resume(
                        &clients.redis,
                        cold_cache.guild_chunks,
                        cold_cache.user_chunks,
                    )
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
        }
    }
    let cluster = Cluster::new(cb.build()).await?;

    // Shard states
    let shard_states = DashMap::with_capacity(shards_per_cluster as usize);
    for i in 0..shards_per_cluster {
        shard_states.insert(i, core::ShardState::PendingCreation);
    }

    // Tracked streams
    let tracked_streams = clients.psql.get_stream_tracks().await?;

    let backend = BackendData {
        cluster,
        shard_states,
        total_shards,
        shards_per_cluster,
    };

    // Final context
    let ctx = Arc::new(
        Context::new(
            cache,
            http,
            clients,
            backend,
            stored_values,
            tracked_streams,
        )
        .await,
    );

    // Setup graceful shutdown
    let shutdown_ctx = ctx.clone();
    ctrlc::set_handler(move || {
        let _ = Runtime::new().unwrap().block_on(async {
            let (store_result, cold_resume_result) = tokio::join!(
                shutdown_ctx.store_values(),
                shutdown_ctx.initiate_cold_resume()
            );
            if let Err(why) = store_result {
                error!("Error while storing values: {}", why);
            }
            if let Err(why) = cold_resume_result {
                error!("Error while freezing cache: {}", why);
            }
        });
        process::exit(0);
    })
    .map_err(|why| format_err!("Failed to register shutdown handler: {}", why))?;

    // Spawn twitch worker
    let twitch_ctx = ctx.clone();
    tokio::spawn(twitch::twitch_loop(twitch_ctx, twitch));

    let c = ctx.backend.cluster.clone();
    tokio::spawn(async move {
        time::delay_for(Duration::from_secs(1)).await;
        c.up().await;
    });
    let mut bot_events = ctx.backend.cluster.events().await;
    let cmd_groups = Arc::new(CommandGroups::new());
    while let Some(event) = bot_events.next().await {
        let (shard, event) = event;
        debug!("Got event, updating stats...");
        ctx.update_stats(shard, &event);
        debug!("Updating cache...");
        ctx.cache.update(shard, &event, ctx.clone()).await?;
        debug!("Updating standby...");
        ctx.standby.process(&event);
        debug!("Pre-event handling done");
        let c = ctx.clone();
        let cmds = cmd_groups.clone();
        tokio::spawn(async move {
            if let Err(why) = handle_event(shard, &event, c, cmds).await {
                error!("Error while handling event: {}", why);
            }
        });
    }
    ctx.backend.cluster.down().await;
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
