mod impls;

use std::{num::NonZeroU32, sync::Arc};

use bb8_redis::{bb8::Pool, RedisConnectionManager};
use dashmap::{DashMap, DashSet};
use hashbrown::HashSet;
use parking_lot::Mutex;
use rosu_v2::Osu;
use smallvec::SmallVec;
use tokio::sync::mpsc::UnboundedSender;
use twilight_gateway::Cluster;
use twilight_http::Client as HttpClient;
use twilight_model::id::{
    marker::{ApplicationMarker, ChannelMarker, GuildMarker, MessageMarker},
    Id,
};
use twilight_standby::Standby;

use crate::{
    commands::fun::GameState,
    core::{buckets::Buckets, BotStats},
    database::{Database, GuildConfig},
    server::AuthenticationStandby,
    util::CountryCode,
    CustomClient, OsuTracking,
};

pub use self::impls::{MatchLiveChannels, MatchTrackResult};

use super::{Cache, RedisCache};

pub type Redis = Pool<RedisConnectionManager>;

pub struct Context {
    pub cache: Cache,
    pub stats: Arc<BotStats>,
    pub http: Arc<HttpClient>,
    pub standby: Standby,
    pub auth_standby: AuthenticationStandby,
    pub buckets: Buckets,
    pub cluster: Cluster,
    pub clients: Clients,
    pub member_requests: MemberRequests,
    // private to avoid deadlocks by messing up references
    data: ContextData,
}

pub struct MemberRequests {
    pub tx: UnboundedSender<(Id<GuildMarker>, u64)>,
    pub todo_guilds: DashSet<Id<GuildMarker>>,
}

pub struct Clients {
    pub psql: Database,
    pub redis: Redis,
    pub osu: Osu,
    pub custom: CustomClient,
}

pub type AssignRoles = SmallVec<[u64; 1]>;

pub struct ContextData {
    // ! CAREFUL: When entries are added or modified
    // ! don't forget to update the DB entry as well
    pub guilds: DashMap<Id<GuildMarker>, GuildConfig>,
    // Mapping twitch user ids to vec of discord channel ids
    pub tracked_streams: DashMap<u64, Vec<u64>>,
    // Mapping (channel id, message id) to role id
    pub role_assigns: DashMap<(u64, u64), AssignRoles>,
    pub bg_games: DashMap<Id<ChannelMarker>, GameState>,
    pub osu_tracking: OsuTracking,
    pub msgs_to_process: DashSet<Id<MessageMarker>>,
    pub map_garbage_collection: Mutex<HashSet<NonZeroU32>>,
    pub match_live: MatchLiveChannels,
    pub snipe_countries: DashMap<CountryCode, String>,
    pub application_id: Id<ApplicationMarker>,
}

impl Context {
    #[cold]
    pub async fn new(
        cache: Cache,
        stats: Arc<BotStats>,
        http: Arc<HttpClient>,
        clients: Clients,
        cluster: Cluster,
        data: ContextData,
        tx: UnboundedSender<(Id<GuildMarker>, u64)>,
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
            member_requests: MemberRequests {
                tx,
                todo_guilds: DashSet::new(),
            },
        }
    }

    pub fn osu(&self) -> &Osu {
        &self.clients.osu
    }

    pub fn psql(&self) -> &Database {
        &self.clients.psql
    }

    pub fn redis(&self) -> RedisCache<'_> {
        RedisCache::new(self)
    }
}
