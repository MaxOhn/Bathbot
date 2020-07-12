use crate::{
    core::{CommandGroups, Context},
    BotResult, Error,
};

use std::sync::{atomic::Ordering, Arc};
use twilight::gateway::Event;

pub async fn handle_event(
    shard_id: u64,
    event: &Event,
    ctx: Arc<Context>,
    cmd_groups: Arc<CommandGroups>,
) -> BotResult<()> {
    match &event {
        // ####################
        // ## Gateway status ##
        // ####################
        Event::ShardReconnecting(_) => info!("Shard {} is attempting to reconnect", shard_id),
        Event::ShardResuming(_) => info!("Shard {} is resuming", shard_id),
        Event::Ready(_) => info!("Shard {} ready to go!", shard_id),
        Event::Resumed => info!("Shard {} successfully resumed", shard_id),
        Event::GatewayReconnect => info!("Gateway requested shard {} to reconnect", shard_id),
        Event::GatewayInvalidateSession(recon) => {
            if *recon {
                warn!(
                    "Gateway has invalidated session for shard {}, but its reconnectable",
                    shard_id
                );
            } else {
                return Err(Error::InvalidSession(shard_id));
            }
        }
        Event::GatewayHello(u) => {
            debug!("Registered with gateway {} on shard {}", u, shard_id);
        }

        // ###########
        // ## Other ##
        // ###########
        Event::MessageCreate(msg) => {
            ctx.stats.new_message(&ctx, msg);
            let prefixes = match msg.guild_id {
                Some(guild_id) => {
                    let guild = ctx.cache.get_guild(guild_id);
                    match guild {
                        Some(g) => {
                            if !g.complete.load(Ordering::SeqCst) {
                                debug!(
                                    "Message received in guild {} but guild not fully cached yet",
                                    g.id
                                );
                                return Ok(()); // not cached yet, just ignore for now
                            }
                        }
                        None => return Ok(()), // we didnt even get a guild create yet
                    }
                    let config = ctx.database.get_guild_config(guild_id.0).await?;
                    config.prefixes.clone()
                }
                None => vec!["<".to_owned(), "!!".to_owned()],
            };

            for p in prefixes {
                if msg.content.starts_with(&p) {
                    // Parser::figure_it_out(&prefix, msg, ctx, shard_id).await?;
                    break;
                }
            }
        }
        _ => (),
    }
    Ok(())
}
