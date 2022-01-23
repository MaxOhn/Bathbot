mod impls;

use bb8_redis::{bb8::Pool, RedisConnectionManager};
use dashmap::{DashMap, DashSet};
use hashbrown::HashSet;
use parking_lot::Mutex;
use rosu_v2::Osu;
use smallvec::SmallVec;
use std::{num::NonZeroU32, sync::Arc};
use tokio::sync::mpsc::UnboundedSender;
use twilight_gateway::Cluster;
use twilight_http::Client as HttpClient;
use twilight_model::{
    gateway::{
        payload::outgoing::UpdatePresence,
        presence::{Activity, ActivityType, Status},
    },
    id::{
        marker::{ApplicationMarker, ChannelMarker, GuildMarker, MessageMarker},
        Id,
    },
};
use twilight_standby::Standby;

use crate::{
    bg_game::GameWrapper,
    core::{buckets::Buckets, BotStats},
    database::{Database, GuildConfig},
    server::AuthenticationStandby,
    util::CountryCode,
    BotResult, CustomClient, OsuTracking, Twitch,
};

pub use self::impls::{MatchLiveChannels, MatchTrackResult};

use super::Cache;

pub struct Context {
    pub cache: Cache,
    pub stats: Arc<BotStats>,
    pub http: Arc<HttpClient>,
    pub standby: Standby,
    pub auth_standby: AuthenticationStandby,
    pub buckets: Buckets,
    pub cluster: Cluster,
    pub clients: Clients,
    pub member_tx: UnboundedSender<(Id<GuildMarker>, u64)>,
    // private to avoid deadlocks by messing up references
    data: ContextData,
}

pub struct Clients {
    pub psql: Database,
    pub redis: Pool<RedisConnectionManager>,
    pub osu: Osu,
    pub custom: CustomClient,
    pub twitch: Twitch,
}

pub type AssignRoles = SmallVec<[u64; 1]>;

pub struct ContextData {
    // ! CAREFUL: When entries are added or modified
    // ! don't forget to update the DB entry aswell
    pub guilds: DashMap<Id<GuildMarker>, GuildConfig>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), AssignRoles>,
    pub bg_games: DashMap<Id<ChannelMarker>, GameWrapper>,
    pub osu_tracking: OsuTracking,
    pub msgs_to_process: DashSet<Id<MessageMarker>>,
    pub map_garbage_collection: Mutex<HashSet<NonZeroU32>>,
    pub match_live: MatchLiveChannels,
    pub snipe_countries: DashMap<CountryCode, String>,
    pub application_id: Id<ApplicationMarker>,
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
        member_tx: UnboundedSender<(Id<GuildMarker>, u64)>,
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
            buckets: Buckets::new(),
            member_tx,
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
