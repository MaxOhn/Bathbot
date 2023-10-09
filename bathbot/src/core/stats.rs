use bathbot_cache::{model::CacheChange, Cache};
use prometheus::{IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry};
use time::OffsetDateTime;
use twilight_gateway::Event;
use twilight_model::application::{
    command::CommandType,
    interaction::{application_command::CommandOptionValue, InteractionData, InteractionType},
};

pub struct EventStats {
    pub channel_create: IntCounter,
    pub channel_delete: IntCounter,
    pub channel_update: IntCounter,
    pub gateway_invalidate: IntCounter,
    pub gateway_reconnect: IntCounter,
    pub guild_create: IntCounter,
    pub guild_delete: IntCounter,
    pub guild_update: IntCounter,
    pub interaction_create: IntCounter,
    pub member_add: IntCounter,
    pub member_remove: IntCounter,
    pub member_update: IntCounter,
    pub member_chunk: IntCounter,
    pub message_create: IntCounter,
    pub message_delete: IntCounter,
    pub message_delete_bulk: IntCounter,
    pub message_update: IntCounter,
    pub role_create: IntCounter,
    pub role_delete: IntCounter,
    pub role_update: IntCounter,
    pub thread_create: IntCounter,
    pub thread_delete: IntCounter,
    pub thread_list_sync: IntCounter,
    pub thread_member_update: IntCounter,
    pub thread_members_update: IntCounter,
    pub thread_update: IntCounter,
    pub unavailable_guild: IntCounter,
    pub user_update: IntCounter,
}

pub struct MessageCounters {
    pub user_messages: IntCounter,
    pub bot_messages: IntCounter,
}

pub struct OsuCounters {
    pub rosu: IntCounterVec,
    pub user_cached: IntCounter,
    pub osutracker_stats_cached: IntCounter,
    pub osutracker_pp_group_cached: IntCounter,
    pub osutracker_id_counts_cached: IntCounter,
    pub osekai_medals_cached: IntCounter,
    pub osekai_badges_cached: IntCounter,
    pub osekai_ranking_cached: IntCounter,
    pub snipe_countries_cached: IntCounter,
    pub cs_diffs_cached: IntCounter,
    pub pp_ranking_cached: IntCounter,
    pub osustats_best_cached: IntCounter,
}

pub struct CommandCounters {
    pub prefix_commands: IntCounterVec,
    pub slash_commands: IntCounterVec,
    pub message_commands: IntCounterVec,
    pub user_commands: IntCounterVec,
    pub components: IntCounterVec,
    pub autocompletes: IntCounterVec,
    pub modals: IntCounterVec,
}

pub struct CacheStats {
    pub channels: IntGauge,
    pub guilds: IntGauge,
    pub roles: IntGauge,
    pub unavailable_guilds: IntGauge,
    pub users: IntGauge,
}

pub struct ClientStats {
    pub github_prs: IntCounter,
}

pub struct BotStats {
    pub start_time: OffsetDateTime,
    pub event_counts: EventStats,
    pub message_counts: MessageCounters,
    pub command_counts: CommandCounters,
    pub cache_counts: CacheStats,
    pub osu_metrics: OsuCounters,
    pub client_counts: ClientStats,
}

macro_rules! metric_vec {
    (counter: $opt:literal, $help:literal, $label:expr) => {
        IntCounterVec::new(Opts::new($opt, $help), &[$label]).unwrap()
    };

    (gauge: $opt:literal, $help:literal, $label:expr) => {
        IntGaugeVec::new(Opts::new($opt, $help), &[$label]).unwrap()
    };
}

impl BotStats {
    pub fn new(osu_metrics: IntCounterVec) -> (Self, Registry) {
        let event_counter = metric_vec!(counter: "gateway_events", "Gateway events", "events");
        let msg_counter = metric_vec!(counter: "messages", "Received messages", "sender_type");
        let prefix_commands =
            metric_vec!(counter: "prefix_commands", "Executed prefix commands", "name");
        let slash_commands = IntCounterVec::new(
            Opts::new("slash_commands", "Executed slash commands"),
            &["name", "group", "sub"],
        )
        .unwrap();
        let message_commands =
            metric_vec!(counter: "message_commands", "Executed message commands", "name");
        let user_commands = metric_vec!(counter: "user_commands", "Executed user commands", "name");
        let components =
            metric_vec!(counter: "components", "Executed interaction components", "name");
        let autocompletes =
            metric_vec!(counter: "autocompletes", "Executed command autocompletes", "name");
        let modals = metric_vec!(counter: "modals", "Executed modals", "name");
        let cache_counter = metric_vec!(gauge: "cache", "Cache counts", "cached_type");
        let client_counts = metric_vec!(counter: "client", "Client requests", "request");

        let registry = Registry::new_custom(Some(String::from("bathbot")), None).unwrap();
        registry.register(Box::new(event_counter.clone())).unwrap();
        registry.register(Box::new(msg_counter.clone())).unwrap();
        registry
            .register(Box::new(prefix_commands.clone()))
            .unwrap();
        registry.register(Box::new(slash_commands.clone())).unwrap();
        registry.register(Box::new(components.clone())).unwrap();
        registry.register(Box::new(autocompletes.clone())).unwrap();
        registry.register(Box::new(modals.clone())).unwrap();
        registry.register(Box::new(cache_counter.clone())).unwrap();
        registry.register(Box::new(osu_metrics.clone())).unwrap();
        registry.register(Box::new(client_counts.clone())).unwrap();

        let stats = Self {
            start_time: OffsetDateTime::now_utc(),
            event_counts: EventStats {
                channel_create: event_counter.with_label_values(&["ChannelCreate"]),
                channel_delete: event_counter.with_label_values(&["ChannelDelete"]),
                channel_update: event_counter.with_label_values(&["ChannelUpdate"]),
                gateway_invalidate: event_counter.with_label_values(&["GatewayInvalidate"]),
                gateway_reconnect: event_counter.with_label_values(&["GatewayReconnect"]),
                guild_create: event_counter.with_label_values(&["GuildCreate"]),
                guild_delete: event_counter.with_label_values(&["GuildDelete"]),
                guild_update: event_counter.with_label_values(&["GuildUpdate"]),
                interaction_create: event_counter.with_label_values(&["InteractionCreate"]),
                member_add: event_counter.with_label_values(&["MemberAdd"]),
                member_remove: event_counter.with_label_values(&["MemberRemove"]),
                member_update: event_counter.with_label_values(&["MemberUpdate"]),
                member_chunk: event_counter.with_label_values(&["MemberChunk"]),
                message_create: event_counter.with_label_values(&["MessageCreate"]),
                message_delete: event_counter.with_label_values(&["MessageDelete"]),
                message_delete_bulk: event_counter.with_label_values(&["MessageDeleteBulk"]),
                message_update: event_counter.with_label_values(&["MessageUpdate"]),
                role_create: event_counter.with_label_values(&["RoleCreate"]),
                role_delete: event_counter.with_label_values(&["RoleDelete"]),
                role_update: event_counter.with_label_values(&["RoleUpdate"]),
                unavailable_guild: event_counter.with_label_values(&["UnavailableGuild"]),
                thread_create: event_counter.with_label_values(&["ThreadCreate"]),
                thread_delete: event_counter.with_label_values(&["ThreadDelete"]),
                thread_list_sync: event_counter.with_label_values(&["ThreadListSync"]),
                thread_member_update: event_counter.with_label_values(&["ThreadMemberUpdate"]),
                thread_members_update: event_counter.with_label_values(&["ThreadMembersUpdate"]),
                thread_update: event_counter.with_label_values(&["ThreadUpdate"]),
                user_update: event_counter.with_label_values(&["UserUpdate"]),
            },
            message_counts: MessageCounters {
                user_messages: msg_counter.with_label_values(&["User"]),
                bot_messages: msg_counter.with_label_values(&["Bot"]),
            },
            command_counts: CommandCounters {
                prefix_commands,
                slash_commands,
                message_commands,
                user_commands,
                components,
                autocompletes,
                modals,
            },
            cache_counts: CacheStats {
                channels: cache_counter.with_label_values(&["Channels"]),
                guilds: cache_counter.with_label_values(&["Guilds"]),
                roles: cache_counter.with_label_values(&["Roles"]),
                unavailable_guilds: cache_counter.with_label_values(&["Unavailable guilds"]),
                users: cache_counter.with_label_values(&["Users"]),
            },
            osu_metrics: OsuCounters {
                user_cached: osu_metrics.with_label_values(&["User cached"]),
                osutracker_stats_cached: osu_metrics
                    .with_label_values(&["osutracker stats cached"]),
                osutracker_pp_group_cached: osu_metrics
                    .with_label_values(&["osutracker pp group cached"]),
                osutracker_id_counts_cached: osu_metrics
                    .with_label_values(&["osutracker id counts cached"]),
                osekai_medals_cached: osu_metrics.with_label_values(&["Medals cached"]),
                osekai_badges_cached: osu_metrics.with_label_values(&["Badges cached"]),
                osekai_ranking_cached: osu_metrics.with_label_values(&["Osekai ranking cached"]),
                pp_ranking_cached: osu_metrics.with_label_values(&["Rankings cached"]),
                snipe_countries_cached: osu_metrics.with_label_values(&["Snipe countries cached"]),
                osustats_best_cached: osu_metrics.with_label_values(&["osustats best cached"]),
                cs_diffs_cached: osu_metrics.with_label_values(&["Cached cs difficulties"]),
                rosu: osu_metrics,
            },
            client_counts: ClientStats {
                github_prs: client_counts.with_label_values(&["github"]),
            },
        };

        (stats, registry)
    }

    pub async fn populate(&self, cache: &Cache) {
        let stats = cache.stats();

        self.cache_counts.guilds.set(stats.guilds as i64);
        self.cache_counts
            .unavailable_guilds
            .set(stats.unavailable_guilds as i64);
        self.cache_counts.channels.set(stats.channels as i64);
        self.cache_counts.users.set(stats.users as i64);
        self.cache_counts.roles.set(stats.roles as i64);
    }

    pub fn increment_message_command(&self, cmd: &str) {
        self.command_counts
            .prefix_commands
            .with_label_values(&[cmd])
            .inc();
    }

    pub fn inc_cached_user(&self) {
        self.osu_metrics.user_cached.inc();
    }

    pub fn inc_cached_osutracker_stats(&self) {
        self.osu_metrics.osutracker_stats_cached.inc();
    }

    pub fn inc_cached_osutracker_pp_group(&self) {
        self.osu_metrics.osutracker_pp_group_cached.inc();
    }

    pub fn inc_cached_osutracker_counts(&self) {
        self.osu_metrics.osutracker_id_counts_cached.inc();
    }

    pub fn inc_cached_medals(&self) {
        self.osu_metrics.osekai_medals_cached.inc();
    }

    pub fn inc_cached_badges(&self) {
        self.osu_metrics.osekai_badges_cached.inc();
    }

    pub fn inc_cached_pp_ranking(&self) {
        self.osu_metrics.pp_ranking_cached.inc();
    }

    pub fn inc_cached_osekai_ranking(&self) {
        self.osu_metrics.osekai_ranking_cached.inc();
    }

    pub fn inc_cached_snipe_countries(&self) {
        self.osu_metrics.snipe_countries_cached.inc();
    }

    pub fn inc_cached_osustats_best(&self) {
        self.osu_metrics.osustats_best_cached.inc();
    }

    pub fn inc_cached_cs_diffs(&self) {
        self.osu_metrics.cs_diffs_cached.inc();
    }

    pub fn inc_cached_github_prs(&self) {
        self.client_counts.github_prs.inc();
    }

    pub fn process(&self, event: &Event, change: Option<CacheChange>) {
        if let Some(change) = change {
            self.cache_counts.channels.add(change.channels as i64);
            self.cache_counts.guilds.add(change.guilds as i64);
            self.cache_counts.roles.add(change.roles as i64);
            self.cache_counts
                .unavailable_guilds
                .add(change.unavailable_guilds as i64);
            self.cache_counts.users.add(change.users as i64);
        }

        match event {
            Event::ChannelCreate(_) => self.event_counts.channel_create.inc(),
            Event::ChannelDelete(_) => self.event_counts.channel_delete.inc(),
            Event::ChannelUpdate(_) => self.event_counts.channel_update.inc(),
            Event::GatewayInvalidateSession(_) => self.event_counts.gateway_invalidate.inc(),
            Event::GatewayReconnect => self.event_counts.gateway_reconnect.inc(),
            Event::GuildCreate(_) => self.event_counts.guild_create.inc(),
            Event::GuildDelete(_) => self.event_counts.guild_delete.inc(),
            Event::GuildUpdate(_) => self.event_counts.guild_update.inc(),
            Event::InteractionCreate(e) => {
                self.event_counts.interaction_create.inc();

                match &e.data {
                    Some(InteractionData::ApplicationCommand(data)) => {
                        if let InteractionType::ApplicationCommandAutocomplete = e.kind {
                            self.command_counts
                                .autocompletes
                                .with_label_values(&[&data.name])
                                .inc()
                        } else {
                            match data.kind {
                                CommandType::ChatInput => {
                                    let (group, sub) = match data.options.first() {
                                        Some(option) => match option.value {
                                            CommandOptionValue::SubCommand(_) => {
                                                ("", option.name.as_str())
                                            }
                                            CommandOptionValue::SubCommandGroup(ref vec) => {
                                                match vec.first() {
                                                    Some(sub) => {
                                                        (option.name.as_str(), sub.name.as_str())
                                                    }
                                                    None => (option.name.as_str(), ""),
                                                }
                                            }
                                            _ => ("", ""),
                                        },
                                        None => ("", ""),
                                    };

                                    self.command_counts
                                        .slash_commands
                                        .with_label_values(&[&data.name, group, sub])
                                        .inc()
                                }
                                CommandType::User => self
                                    .command_counts
                                    .user_commands
                                    .with_label_values(&[&data.name])
                                    .inc(),
                                CommandType::Message => self
                                    .command_counts
                                    .prefix_commands
                                    .with_label_values(&[&data.name])
                                    .inc(),
                                _ => {}
                            }
                        }
                    }
                    Some(InteractionData::MessageComponent(data)) => self
                        .command_counts
                        .components
                        .with_label_values(&[&data.custom_id])
                        .inc(),
                    Some(InteractionData::ModalSubmit(data)) => self
                        .command_counts
                        .modals
                        .with_label_values(&[&data.custom_id])
                        .inc(),
                    _ => {}
                }
            }
            Event::MemberAdd(_) => self.event_counts.member_add.inc(),
            Event::MemberRemove(_) => self.event_counts.member_remove.inc(),
            Event::MemberUpdate(_) => self.event_counts.member_update.inc(),
            Event::MemberChunk(_) => self.event_counts.member_chunk.inc(),
            Event::MessageCreate(msg) => {
                self.event_counts.message_create.inc();

                if msg.author.bot {
                    self.message_counts.bot_messages.inc()
                } else {
                    self.message_counts.user_messages.inc()
                }
            }
            Event::MessageDelete(_) => self.event_counts.message_delete.inc(),
            Event::MessageDeleteBulk(_) => self.event_counts.message_delete_bulk.inc(),
            Event::MessageUpdate(_) => self.event_counts.message_update.inc(),
            Event::Ready(_) => {}
            Event::RoleCreate(_) => self.event_counts.role_create.inc(),
            Event::RoleDelete(_) => self.event_counts.role_delete.inc(),
            Event::RoleUpdate(_) => self.event_counts.role_update.inc(),
            Event::ThreadCreate(_) => self.event_counts.thread_create.inc(),
            Event::ThreadDelete(_) => self.event_counts.thread_delete.inc(),
            Event::ThreadListSync(_) => self.event_counts.thread_list_sync.inc(),
            Event::ThreadMemberUpdate(_) => self.event_counts.thread_member_update.inc(),
            Event::ThreadMembersUpdate(_) => self.event_counts.thread_members_update.inc(),
            Event::ThreadUpdate(_) => self.event_counts.thread_update.inc(),
            Event::UnavailableGuild(_) => self.event_counts.unavailable_guild.inc(),
            Event::UserUpdate(_) => self.event_counts.user_update.inc(),
            _ => {}
        }
    }
}
