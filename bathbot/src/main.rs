#[macro_use]
extern crate tracing;

#[macro_use]
extern crate eyre;

mod active;
mod commands;
mod core;
mod embeds;
mod manager;
mod tracking;
mod util;

#[cfg(feature = "matchlive")]
mod matchlive;

use bathbot_model::Countries;
use eyre::{Result, WrapErr};
use tokio::{
    runtime::Builder as RuntimeBuilder,
    signal,
    sync::{broadcast, mpsc},
    task::JoinSet,
};

use crate::{
    commands::owner::RESHARD_TX,
    core::{BotConfig, Context, commands::interaction::InteractionCommands, event_loop, logging},
};

fn main() {
    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()
        .expect("Could not build runtime");

    if let Err(err) = dotenvy::dotenv() {
        panic!("Failed to prepare .env variables: {err}");
    }

    let _log_worker_guard = logging::init();

    if let Err(source) = runtime.block_on(async_main()) {
        error!(?source, "Critical error in main");
    }
}

async fn async_main() -> Result<()> {
    // Load config file
    BotConfig::init().context("failed to initialize config")?;
    Countries::init();

    let (member_tx, member_rx) = mpsc::unbounded_channel();

    let res = Context::init(member_tx.clone())
        .await
        .context("Failed to create context")?;

    #[cfg(not(feature = "server"))]
    let (mut shards,) = res;

    #[cfg(feature = "server")]
    let (mut shards, server_tx) = res;

    // Initialize commands
    let slash_commands = InteractionCommands::get().collect();
    info!("Setting {} slash commands...", slash_commands.len());

    if cfg!(feature = "global_slash") {
        let cmds = Context::set_global_commands(slash_commands).await?;
        InteractionCommands::set_ids(&cmds);

        if let Err(err) = Context::set_guild_commands(Vec::new()).await {
            warn!(?err, "Failed to remove guild commands");
        }
    } else {
        let cmds = Context::set_guild_commands(slash_commands).await?;
        InteractionCommands::set_ids(&cmds);

        if let Err(err) = Context::set_global_commands(Vec::new()).await {
            warn!(?err, "Failed to remove global commands");
        }
    }

    #[cfg(feature = "twitchtracking")]
    {
        // Spawn twitch worker
        tokio::spawn(tracking::twitch_tracking_loop());
    }

    #[cfg(feature = "matchlive")]
    {
        // Spawn osu match ticker worker
        tokio::spawn(Context::match_live_loop());
    }

    // Request members
    tokio::spawn(Context::request_guild_members(member_rx));

    let (reshard_tx, reshard_rx) = broadcast::channel(1);

    tokio::spawn(Context::reshard_loop(reshard_tx.clone()));

    RESHARD_TX
        .set(reshard_tx)
        .expect("RESHARD_TX has already been set");

    let mut runners = JoinSet::new();

    tokio::select! {
        _ = event_loop(&mut runners, &mut shards, reshard_rx) => error!("Event loop ended"),
        res = signal::ctrl_c() => match res {
            Ok(_) => info!("Received Ctrl+C"),
            Err(err) => error!(?err, "Failed to await Ctrl+C"),
        }
    }

    #[cfg(feature = "server")]
    if server_tx.send(()).is_err() {
        error!("Failed to send shutdown message to server");
    }

    tokio::select! {
        _ = Context::shutdown(runners, shards) => info!("Shutting down"),
        res = signal::ctrl_c() => match res {
            Ok(_) => info!("Forcing shutdown"),
            Err(err) => error!(?err, "Failed to await second Ctrl+C"),
        }
    }

    Ok(())
}
