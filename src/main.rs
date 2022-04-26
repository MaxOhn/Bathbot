#![deny(clippy::all, nonstandard_style, rust_2018_idioms, unused, warnings)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate tracing;

#[macro_use]
mod error;

mod commands;
mod core;
mod custom_client;
mod database;
mod embeds;
mod games;
mod matchlive;
mod pagination;
mod pp;
mod server;
mod tracking;
mod util;

use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use eyre::{Report, Result, WrapErr};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    signal,
    sync::{mpsc, oneshot},
    time::{self, MissedTickBehavior},
};
use twilight_model::gateway::payload::outgoing::RequestGuildMembers;

use crate::{
    core::{
        commands::{prefix::PREFIX_COMMANDS, slash::SLASH_COMMANDS},
        event_loop, logging, Context, CONFIG,
    },
    database::Database,
    error::Error,
};

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
    dotenv::dotenv()?;
    let _log_worker_guard = logging::initialize();

    // Load config file
    core::BotConfig::init().context("failed to initialize config")?;

    let (member_tx, mut member_rx) = mpsc::unbounded_channel();

    let (ctx, events) = Context::new(member_tx.clone())
        .await
        .context("failed to create ctx")?;

    let ctx = Arc::new(ctx);

    // Initialize commands
    PREFIX_COMMANDS.init();
    let slash_commands = SLASH_COMMANDS.collect();
    info!("Setting {} slash commands...", slash_commands.len());

    // info!("Defining: {slash_commands:#?}");

    if cfg!(debug_assertions) {
        ctx.interaction()
            .set_global_commands(&[])
            .exec()
            .await
            .context("failed to set empty global commands")?;

        let _received = ctx
            .interaction()
            .set_guild_commands(CONFIG.get().unwrap().dev_guild, &slash_commands)
            .exec()
            .await
            .context("failed to set guild commands")?;

        // let commands = _received.models().await?;
        // info!("Received: {commands:#?}");
    } else {
        ctx.interaction()
            .set_global_commands(&slash_commands)
            .exec()
            .await
            .context("failed to set global commands")?;
    }

    // Spawn server worker
    let server_ctx = Arc::clone(&ctx);
    let (tx, rx) = oneshot::channel();
    tokio::spawn(server::run_server(server_ctx, rx));

    // Spawn twitch worker
    let twitch_ctx = Arc::clone(&ctx);
    tokio::spawn(tracking::twitch_tracking_loop(twitch_ctx));

    // Spawn osu tracking worker
    let osu_tracking_ctx = Arc::clone(&ctx);
    tokio::spawn(tracking::osu_tracking_loop(osu_tracking_ctx));

    // Spawn background loop worker
    let background_ctx = Arc::clone(&ctx);
    tokio::spawn(Context::background_loop(background_ctx));

    // Spawn osu match ticker worker
    let match_live_ctx = Arc::clone(&ctx);
    tokio::spawn(Context::match_live_loop(match_live_ctx));

    // Request members
    let member_ctx = Arc::clone(&ctx);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(600));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await;
        let mut counter = 1;
        info!("Processing member request queue...");

        while let Some((guild_id, shard_id)) = member_rx.recv().await {
            let removed_opt = member_ctx.member_requests.todo_guilds.remove(&guild_id);

            // If a guild is in the channel twice, only process the first and ignore the second
            if removed_opt.is_none() {
                continue;
            }

            interval.tick().await;
            let req = RequestGuildMembers::builder(guild_id).query("", None);
            trace!("Member request #{counter} for guild {guild_id}");
            counter += 1;

            let command_result = member_ctx
                .cluster
                .command(shard_id, &req)
                .await
                .wrap_err_with(|| format!("failed to request members for guild {guild_id}"));

            if let Err(report) = command_result {
                warn!("{report:?}");

                if let Err(err) = member_tx.send((guild_id, shard_id)) {
                    warn!("Failed to re-forward member request: {err}");
                }
            }
        }
    });

    let event_ctx = Arc::clone(&ctx);
    ctx.cluster.up().await;

    tokio::select! {
        _ = event_loop(event_ctx, events) => error!("Event loop ended"),
        res = signal::ctrl_c() => if let Err(report) = res.wrap_err("error while awaiting ctrl+c") {
            error!("{report:?}");
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

    let count = ctx.stop_all_games().await;
    info!("Stopped {count} bg games");

    let count = ctx.notify_match_live_shutdown().await;
    info!("Stopped match tracking in {count} channels");

    let resume_data = ctx.cluster.down_resumable();

    if let Err(err) = ctx.cache.freeze(ctx.redis_client(), resume_data).await {
        let report = Report::new(err).wrap_err("failed to freeze cache");
        error!("{report:?}");
    }

    let (count, total) = ctx.garbage_collect_all_maps().await;
    info!("Garbage collected {count}/{total} maps");

    info!("Shutting down");

    Ok(())
}
