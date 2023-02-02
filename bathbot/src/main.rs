#![deny(clippy::all, nonstandard_style, rust_2018_idioms, unused, warnings)]

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate eyre;

mod commands;
mod core;
mod embeds;
mod games;
mod manager;
mod matchlive;
mod pagination;
mod tracking;
mod util;

use std::{sync::Arc, time::Duration};

use eyre::{Report, Result, WrapErr};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    signal,
    sync::mpsc,
    time::{self, MissedTickBehavior},
};
use twilight_model::gateway::payload::outgoing::RequestGuildMembers;

use crate::core::{commands::slash::SlashCommands, event_loop, logging, BotConfig, Context};

fn main() {
    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .thread_stack_size(3 * 1024 * 1024)
        .build()
        .expect("Could not build runtime");

    if dotenvy::dotenv().is_err() {
        panic!(
            "Failed to parse .env file. \
            Be sure there is one in the same folder as this executable."
        );
    }

    let _log_worker_guard = logging::init();

    if let Err(report) = runtime.block_on(async_main()) {
        error!("{:?}", report.wrap_err("Critical error in main"));
    }
}

async fn async_main() -> Result<()> {
    // Load config file
    BotConfig::init().context("failed to initialize config")?;

    let (member_tx, mut member_rx) = mpsc::unbounded_channel();

    let tuple = Context::new(member_tx.clone())
        .await
        .context("failed to create ctx")?;

    #[cfg(not(feature = "server"))]
    let (ctx, events) = tuple;

    #[cfg(feature = "server")]
    let (ctx, events, server_tx) = tuple;

    let ctx = Arc::new(ctx);

    // Initialize commands
    let slash_commands = SlashCommands::get().collect();
    info!("Setting {} slash commands...", slash_commands.len());
    let interaction_client = ctx.interaction();

    #[cfg(feature = "global_slash")]
    {
        interaction_client
            .set_global_commands(&slash_commands)
            .exec()
            .await
            .context("failed to set global commands")?;

        let guild_command_fut = interaction_client
            .set_guild_commands(BotConfig::get().dev_guild, &[])
            .exec();

        if let Err(err) = guild_command_fut.await {
            let wrap = "Failed to remove guild commands";
            warn!("{:?}", Report::new(err).wrap_err(wrap));
        }
    }

    #[cfg(not(feature = "global_slash"))]
    {
        interaction_client
            .set_guild_commands(BotConfig::get().dev_guild, &slash_commands)
            .exec()
            .await
            .context("failed to set guild commands")?;

        let global_command_fut = interaction_client.set_global_commands(&[]).exec();

        if let Err(err) = global_command_fut.await {
            let wrap = "Failed to remove global commands";
            warn!("{:?}", Report::new(err).wrap_err(wrap));
        }
    }

    #[cfg(feature = "twitchtracking")]
    {
        // Spawn twitch worker
        let twitch_ctx = Arc::clone(&ctx);
        tokio::spawn(tracking::twitch_tracking_loop(twitch_ctx));
    }

    #[cfg(feature = "osutracking")]
    {
        // Spawn osu tracking worker
        let osu_tracking_ctx = Arc::clone(&ctx);
        tokio::spawn(tracking::osu_tracking_loop(osu_tracking_ctx));
    }

    #[cfg(feature = "matchlive")]
    {
        // Spawn osu match ticker worker
        let match_live_ctx = Arc::clone(&ctx);
        tokio::spawn(Context::match_live_loop(match_live_ctx));
    }

    // Request members
    let member_ctx = Arc::clone(&ctx);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(600));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        interval.tick().await;
        let mut counter = 1;
        info!("Processing member request queue...");

        while let Some((guild_id, shard_id)) = member_rx.recv().await {
            let removed_opt = member_ctx
                .member_requests
                .todo_guilds
                .lock()
                .remove(&guild_id);

            // If a guild is in the channel twice, only process the first and ignore the second
            if !removed_opt {
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

            if let Err(err) = command_result {
                warn!("{err:?}");

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
        res = signal::ctrl_c() => match res {
            Ok(_) => info!("Received Ctrl+C"),
            Err(err) => {
                let err = Report::new(err).wrap_err("Failed to await Ctrl+C");
                error!("{err:?}");
            }
        }
    }

    #[cfg(feature = "server")]
    if server_tx.send(()).is_err() {
        error!("Failed to send shutdown message to server");
    }

    // Disable tracking while preparing shutdown
    #[cfg(feature = "osutracking")]
    ctx.tracking().set_tracking(true);

    // Prevent non-minimized msgs from getting minimized
    ctx.clear_msgs_to_process();

    let count = ctx.stop_all_games().await;
    info!("Stopped {count} bg games");

    #[cfg(feature = "matchlive")]
    {
        let count = ctx.notify_match_live_shutdown().await;
        info!("Stopped match tracking in {count} channels");
    }

    let resume_data = ctx.cluster.down_resumable();

    if let Err(err) = ctx.cache.freeze(&ctx, resume_data).await {
        let report = Report::new(err).wrap_err("Failed to freeze cache");
        error!("{report:?}");
    }

    info!("Shutting down");

    Ok(())
}
