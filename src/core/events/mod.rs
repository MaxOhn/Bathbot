use std::{
    fmt::{Display, Error, Formatter},
    sync::Arc,
};

use futures::StreamExt;
use twilight_gateway::{cluster::Events, Event};
use twilight_model::id::Id;

use crate::{util::Authored, BotResult};

use self::{interaction::handle_interaction, message::handle_message};

use super::{buckets::BucketName, Context};

mod interaction;
mod message;

#[derive(Debug)]
enum ProcessResult {
    Success,
    NoDM,
    NoSendPermission,
    Ratelimited(BucketName),
    NoOwner,
    NoAuthority,
}

fn log_command(ctx: &Context, cmd: &dyn Authored, name: &str) {
    let username = cmd
        .user()
        .map(|u| u.username.as_str())
        .unwrap_or("<unknown user>");

    let location = CommandLocation { ctx, cmd };
    info!("[{location}] {username} invoked `{name}`");
}

struct CommandLocation<'a> {
    ctx: &'a Context,
    cmd: &'a dyn Authored,
}

impl Display for CommandLocation<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        let guild = match self.cmd.guild_id() {
            Some(id) => id,
            None => return f.write_str("Private"),
        };

        match self.ctx.cache.guild(guild, |g| write!(f, "{}:", g.name())) {
            Ok(Ok(_)) => {
                let channel_res = self.ctx.cache.channel(self.cmd.channel_id(), |c| {
                    f.write_str(c.name.as_deref().unwrap_or("<uncached channel>"))
                });

                match channel_res {
                    Ok(Ok(_)) => Ok(()),
                    Ok(err) => err,
                    Err(_) => f.write_str("<uncached channel>"),
                }
            }
            Ok(err) => err,
            Err(_) => f.write_str("<uncached guild>"),
        }
    }
}

pub async fn event_loop(ctx: Arc<Context>, mut events: Events) {
    while let Some((shard_id, event)) = events.next().await {
        ctx.cache.update(&event);
        ctx.standby.process(&event);
        let ctx = Arc::clone(&ctx);

        tokio::spawn(async move {
            let handle_fut = handle_event(ctx, event, shard_id);

            if let Err(report) = handle_fut.await.wrap_err("error while handling event") {
                error!("{report:?}");
            }
        });
    }
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
                    "Gateway has invalidated session for shard {shard_id}, but its reconnectable"
                );
            } else {
                warn!("Gateway has invalidated session for shard {shard_id}");
            }
        }
        Event::GatewayReconnect => {
            info!("Gateway requested shard {shard_id} to reconnect");
            ctx.stats.event_counts.gateway_reconnect.inc();
        }
        Event::GiftCodeUpdate => {}
        Event::GuildCreate(e) => {
            ctx.stats.event_counts.guild_create.inc();
            ctx.member_requests.todo_guilds.insert(e.id);

            if let Err(why) = ctx.member_requests.tx.send((e.id, shard_id)) {
                warn!("Failed to forward member request: {why}");
            }

            let stats = ctx.cache.stats();
            ctx.stats.cache_counts.guilds.set(stats.guilds() as i64);
            ctx.stats
                .cache_counts
                .unavailable_guilds
                .set(stats.unavailable_guilds() as i64);
            ctx.stats.cache_counts.members.set(stats.members() as i64);
            ctx.stats.cache_counts.users.set(stats.users() as i64);
            ctx.stats.cache_counts.roles.set(stats.roles() as i64);
        }
        Event::GuildDelete(_) => {
            ctx.stats.event_counts.guild_delete.inc();

            let stats = ctx.cache.stats();
            ctx.stats.cache_counts.guilds.set(stats.guilds() as i64);
            ctx.stats
                .cache_counts
                .unavailable_guilds
                .set(stats.unavailable_guilds() as i64);
            ctx.stats.cache_counts.members.set(stats.members() as i64);
            ctx.stats.cache_counts.users.set(stats.users() as i64);
            ctx.stats.cache_counts.roles.set(stats.roles() as i64);
        }
        Event::GuildEmojisUpdate(_) => {}
        Event::GuildIntegrationsUpdate(_) => {}
        Event::GuildStickersUpdate(_) => {}
        Event::GuildUpdate(_) => ctx.stats.event_counts.guild_update.inc(),
        Event::IntegrationCreate(_) => {}
        Event::IntegrationDelete(_) => {}
        Event::IntegrationUpdate(_) => {}
        Event::InteractionCreate(e) => {
            ctx.stats.event_counts.interaction_create.inc();

            handle_interaction(ctx, e.0).await
        }
        Event::InviteCreate(_) => {}
        Event::InviteDelete(_) => {}
        Event::MemberAdd(_) => {
            ctx.stats.event_counts.member_add.inc();

            let stats = ctx.cache.stats();
            ctx.stats.cache_counts.members.set(stats.members() as i64);
            ctx.stats.cache_counts.users.set(stats.users() as i64);
        }
        Event::MemberRemove(_) => {
            ctx.stats.event_counts.member_remove.inc();

            let stats = ctx.cache.stats();
            ctx.stats.cache_counts.members.set(stats.members() as i64);
            ctx.stats.cache_counts.users.set(stats.users() as i64);
        }
        Event::MemberUpdate(_) => ctx.stats.event_counts.member_update.inc(),
        Event::MemberChunk(_) => ctx.stats.event_counts.member_chunk.inc(),
        Event::MessageCreate(msg) => {
            ctx.stats.event_counts.message_create.inc();

            if !msg.author.bot {
                ctx.stats.message_counts.user_messages.inc()
            } else if ctx.cache.is_own(&*msg).await {
                ctx.stats.message_counts.own_messages.inc()
            } else {
                ctx.stats.message_counts.other_bot_messages.inc()
            }

            handle_message(ctx, msg.0).await;
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
                if let Some(roles) = ctx.get_role_assigns(reaction) {
                    for role_id in roles.into_iter().map(Id::new) {
                        let add_role_fut =
                            ctx.http
                                .add_guild_member_role(guild_id, reaction.user_id, role_id);

                        match add_role_fut.exec().await {
                            Ok(_) => debug!("Assigned react-role to user"),
                            Err(why) => error!("Error while assigning react-role to user: {why}"),
                        }
                    }
                }
            }
        }
        Event::ReactionRemove(reaction_remove) => {
            ctx.stats.event_counts.reaction_remove.inc();
            let reaction = &reaction_remove.0;

            if let Some(guild_id) = reaction.guild_id {
                if let Some(roles) = ctx.get_role_assigns(reaction) {
                    for role_id in roles.into_iter().map(Id::new) {
                        let remove_role_fut =
                            ctx.http
                                .remove_guild_member_role(guild_id, reaction.user_id, role_id);

                        match remove_role_fut.exec().await {
                            Ok(_) => debug!("Removed react-role from user"),
                            Err(why) => {
                                error!("Error while removing react-role from user: {why}")
                            }
                        }
                    }
                }
            }
        }
        Event::ReactionRemoveAll(_) => ctx.stats.event_counts.reaction_remove_all.inc(),
        Event::ReactionRemoveEmoji(_) => ctx.stats.event_counts.reaction_remove_emoji.inc(),
        Event::Ready(_) => {
            info!("Shard {shard_id} is ready");

            let stats = ctx.cache.stats();
            ctx.stats.cache_counts.guilds.set(stats.guilds() as i64);
            ctx.stats
                .cache_counts
                .unavailable_guilds
                .set(stats.unavailable_guilds() as i64);
            ctx.stats.cache_counts.members.set(stats.members() as i64);
            ctx.stats.cache_counts.users.set(stats.users() as i64);
            ctx.stats.cache_counts.roles.set(stats.roles() as i64);
        }
        Event::Resumed => info!("Shard {shard_id} is resumed"),
        Event::RoleCreate(_) => {
            ctx.stats.event_counts.role_create.inc();
            ctx.stats
                .cache_counts
                .roles
                .set(ctx.cache.stats().roles() as i64);
        }
        Event::RoleDelete(_) => {
            ctx.stats.event_counts.role_delete.inc();
            ctx.stats
                .cache_counts
                .roles
                .set(ctx.cache.stats().roles() as i64);
        }
        Event::RoleUpdate(_) => ctx.stats.event_counts.role_update.inc(),
        Event::ShardConnected(_) => info!("Shard {shard_id} is connected"),
        Event::ShardConnecting(_) => info!("Shard {shard_id} is connecting..."),
        Event::ShardDisconnected(_) => info!("Shard {shard_id} is disconnected"),
        Event::ShardIdentifying(_) => info!("Shard {shard_id} is identifying..."),
        Event::ShardReconnecting(_) => info!("Shard {shard_id} is reconnecting..."),
        Event::ShardPayload(_) => {}
        Event::ShardResuming(_) => info!("Shard {shard_id} is resuming..."),
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
        Event::UnavailableGuild(_) => {
            ctx.stats.event_counts.unavailable_guild.inc();

            ctx.stats
                .cache_counts
                .unavailable_guilds
                .set(ctx.cache.stats().unavailable_guilds() as i64);
        }
        Event::UserUpdate(_) => ctx.stats.event_counts.user_update.inc(),
        Event::VoiceServerUpdate(_) => {}
        Event::VoiceStateUpdate(_) => {}
        Event::WebhooksUpdate(_) => {}
    }

    Ok(())
}
