mod impls;

pub use impls::{MatchLiveChannels, MatchTrackResult};

use bathbot_cache::Cache;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    bg_game::GameWrapper,
    core::{
        buckets::{buckets, Buckets},
        BotStats,
    },
    database::{Database, GuildConfig},
    util::CountryCode,
    BotResult, CustomClient, OsuTracking, Twitch,
};

use super::server::AuthenticationStandby;

use dashmap::{DashMap, DashSet};
use deadpool_redis::Pool as RedisPool;
use hashbrown::HashSet;
use parking_lot::{Mutex, RwLock};
use rosu_v2::Osu;
use std::sync::Arc;
use twilight_gateway::Cluster;
use twilight_http::Client as HttpClient;
use twilight_model::{
    gateway::{
        payload::outgoing::UpdatePresence,
        presence::{Activity, ActivityType, Status},
    },
    id::{ChannelId, GuildId, MessageId},
    user::CurrentUser,
};
use twilight_standby::Standby;

pub struct Context {
    pub cache: Cache,
    pub stats: Arc<BotStats>,
    pub http: Arc<HttpClient>,
    pub standby: Standby,
    pub auth_standby: AuthenticationStandby,
    pub buckets: Buckets,
    pub cluster: Cluster,
    pub clients: Clients,
    pub member_tx: UnboundedSender<(GuildId, u64)>,
    pub current_user: RwLock<CurrentUser>,
    // private to avoid deadlocks by messing up references
    data: ContextData,
}

pub struct Clients {
    pub psql: Database,
    pub redis: RedisPool,
    pub osu: Osu,
    pub custom: CustomClient,
    pub twitch: Twitch,
}

pub struct ContextData {
    // ! CAREFUL: When entries are added or modified
    // ! don't forget to update the DB entry aswell
    pub guilds: DashMap<GuildId, GuildConfig>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), u64>,
    pub bg_games: DashMap<ChannelId, GameWrapper>,
    pub osu_tracking: OsuTracking,
    pub msgs_to_process: DashSet<MessageId>,
    pub map_garbage_collection: Mutex<HashSet<u32>>,
    pub match_live: MatchLiveChannels,
    pub snipe_countries: DashMap<CountryCode, String>,
}

impl Context {
    #[allow(clippy::too_many_arguments)]
    #[cold]
    pub async fn new(
        cache: Cache,
        stats: Arc<BotStats>,
        http: Arc<HttpClient>,
        clients: Clients,
        cluster: Cluster,
        data: ContextData,
        member_tx: UnboundedSender<(GuildId, u64)>,
        current_user: CurrentUser,
    ) -> Self {
        Context {
            cache,
            stats,
            http,
            standby: Standby::new(),
            auth_standby: AuthenticationStandby::default(),
            clients,
            cluster,
            data,
            buckets: buckets(),
            member_tx,
            current_user: RwLock::new(current_user),
        }
    }

    pub async fn set_cluster_activity<M>(
        &self,
        status: Status,
        activity_type: ActivityType,
        message: M,
    ) -> BotResult<()>
    where
        M: Into<String> + Clone,
    {
        let mut shards = self.cluster.shards();

        if let Some(shard) = shards.next() {
            for shard in shards {
                match shard.info() {
                    Ok(info) => info!("Setting activity for shard {}", info.id()),
                    Err(_) => continue,
                }

                let activities = vec![generate_activity(activity_type, message.clone().into())];
                let status = UpdatePresence::new(activities, false, None, status).unwrap();
                shard.command(&status).await?;
            }

            // Handle last shard separately so the message is not cloned
            if let Ok(info) = shard.info() {
                info!("Setting activity for shard {}", info.id());
                let activities = vec![generate_activity(activity_type, message.into())];
                let status = UpdatePresence::new(activities, false, None, status).unwrap();
                shard.command(&status).await?;
            }
        }

        Ok(())
    }

    pub async fn set_shard_activity(
        &self,
        shard_id: u64,
        status: Status,
        activity_type: ActivityType,
        message: impl Into<String>,
    ) -> BotResult<()> {
        let activities = vec![generate_activity(activity_type, message.into())];
        let status = UpdatePresence::new(activities, false, None, status).unwrap();
        self.cluster.command(shard_id, &status).await?;

        Ok(())
    }

    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }
}

pub fn generate_activity(activity_type: ActivityType, message: String) -> Activity {
    Activity {
        assets: None,
        application_id: None,
        buttons: Vec::new(),
        created_at: None,
        details: None,
        flags: None,
        id: None,
        instance: None,
        kind: activity_type,
        name: message,
        emoji: None,
        party: None,
        secrets: None,
        state: None,
        timestamps: None,
        url: None,
    }
}
