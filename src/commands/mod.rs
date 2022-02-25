pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod tracking;
pub mod twitch;
pub mod utility;

use std::{borrow::Cow, iter::Copied};

use eyre::Report;
use radix_trie::{iter::Keys, Trie, TrieCommon};
use rosu_v2::prelude::{GameMode, Username};
use twilight_model::{
    application::command::{
        BaseCommandOptionData, ChannelCommandOptionData, ChoiceCommandOptionData, Command,
        CommandOption, CommandOptionChoice, CommandOptionValue, CommandType, Number,
        NumberCommandOptionData, OptionsCommandOptionData,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    core::{CommandGroup, Context},
    database::OsuData,
    util::{
        constants::{
            common_literals::{CTB, HELP, MANIA, OSU, PROFILE, TAIKO},
            GENERAL_ISSUE,
        },
        matcher, Emote,
    },
    BotResult,
};

use self::{fun::*, osu::*, owner::*, songs::*, tracking::*, twitch::*, utility::*};

fn parse_mode_option(value: &str) -> Option<GameMode> {
    match value {
        OSU => Some(GameMode::STD),
        TAIKO => Some(GameMode::TKO),
        CTB => Some(GameMode::CTB),
        MANIA => Some(GameMode::MNA),
        _ => None,
    }
}

/// Checks if the resolved data contains a user and tries to get the user's `OsuData`
async fn parse_discord(ctx: &Context, user_id: Id<UserMarker>) -> DoubleResultCow<OsuData> {
    match ctx.psql().get_user_osu(user_id).await {
        Ok(Some(osu)) => Ok(Ok(osu)),
        Ok(None) => {
            let content = format!("<@{user_id}> is not linked to an osu profile");

            Ok(Err(content.into()))
        }
        Err(why) => {
            warn!("{:?}", Report::new(why).wrap_err("failed to get osu data"));

            Ok(Err(GENERAL_ISSUE.into()))
        }
    }
}

async fn check_user_mention(ctx: &Context, arg: &str) -> DoubleResultCow<OsuData> {
    match matcher::get_mention_user(arg) {
        Some(user_id) => match parse_discord(ctx, user_id).await? {
            Ok(osu) => Ok(Ok(osu)),
            Err(content) => Ok(Err(content)),
        },
        None => Ok(Ok(Username::from(arg).into())),
    }
}

type DoubleResultCow<T> = BotResult<Result<T, Cow<'static, str>>>;

pub fn command_groups() -> [CommandGroup; 11] {
    [
        CommandGroup::new(
            "all osu! modes",
            Emote::Osu,
            vec![
                &LINK_CMD,
                &COMPARE_CMD,
                &SIMULATE_CMD,
                &MAP_CMD,
                &FIX_CMD,
                &MATCHCOSTS_CMD,
                &AVATAR_CMD,
                &MOSTPLAYED_CMD,
                &MOSTPLAYEDCOMMON_CMD,
                &LEADERBOARD_CMD,
                &BELGIANLEADERBOARD_CMD,
                &MEDAL_CMD,
                &MEDALSTATS_CMD,
                &MEDALRECENT_CMD,
                &MEDALSMISSING_CMD,
                &MEDALSCOMMON_CMD,
                &SEARCH_CMD,
                &MATCHLIVE_CMD,
                &MATCHLIVEREMOVE_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!standard",
            Emote::Std,
            vec![
                &RECENT_CMD,
                &TOP_CMD,
                &RECENTBEST_CMD,
                &OSU_CMD,
                &OSUCOMPARE_CMD,
                &PP_CMD,
                &WHATIF_CMD,
                &RANK_CMD,
                &COMMON_CMD,
                &BWS_CMD,
                &RECENTLEADERBOARD_CMD,
                &RECENTBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALS_CMD,
                &OSUSTATSCOUNT_CMD,
                &OSUSTATSLIST_CMD,
                &SIMULATERECENT_CMD,
                &RECENTLIST_CMD,
                &NOCHOKES_CMD,
                &SOTARKS_CMD,
                &MAPPER_CMD,
                &TOPIF_CMD,
                &TOPOLD_CMD,
                &SNIPED_CMD,
                &SNIPEDGAIN_CMD,
                &SNIPEDLOSS_CMD,
                &PLAYERSNIPESTATS_CMD,
                &PLAYERSNIPELIST_CMD,
                &COUNTRYSNIPESTATS_CMD,
                &COUNTRYSNIPELIST_CMD,
                &RANKRANKEDSCORE_CMD,
                &PPRANKING_CMD,
                &RANKEDSCORERANKING_CMD,
                &COUNTRYRANKING_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!mania",
            Emote::Mna,
            vec![
                &RECENTMANIA_CMD,
                &TOPMANIA_CMD,
                &RECENTBESTMANIA_CMD,
                &MANIA_CMD,
                &OSUCOMPAREMANIA_CMD,
                &PPMANIA_CMD,
                &WHATIFMANIA_CMD,
                &RANKMANIA_CMD,
                &COMMONMANIA_CMD,
                &RECENTMANIALEADERBOARD_CMD,
                &RECENTMANIABELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSMANIA_CMD,
                &OSUSTATSCOUNTMANIA_CMD,
                &OSUSTATSLISTMANIA_CMD,
                &SIMULATERECENTMANIA_CMD,
                &RECENTLISTMANIA_CMD,
                &RATIOS_CMD,
                &MAPPERMANIA_CMD,
                &TOPOLDMANIA_CMD,
                &RANKRANKEDSCOREMANIA_CMD,
                &PPRANKINGMANIA_CMD,
                &RANKEDSCORERANKINGMANIA_CMD,
                &COUNTRYRANKINGMANIA_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!taiko",
            Emote::Tko,
            vec![
                &RECENTTAIKO_CMD,
                &TOPTAIKO_CMD,
                &RECENTBESTTAIKO_CMD,
                &TAIKO_CMD,
                &OSUCOMPARETAIKO_CMD,
                &PPTAIKO_CMD,
                &WHATIFTAIKO_CMD,
                &RANKTAIKO_CMD,
                &COMMONTAIKO_CMD,
                &RECENTTAIKOLEADERBOARD_CMD,
                &RECENTTAIKOBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSTAIKO_CMD,
                &OSUSTATSCOUNTTAIKO_CMD,
                &OSUSTATSLISTTAIKO_CMD,
                &SIMULATERECENTTAIKO_CMD,
                &RECENTLISTTAIKO_CMD,
                &NOCHOKESTAIKO_CMD,
                &MAPPERTAIKO_CMD,
                &TOPIFTAIKO_CMD,
                &TOPOLDTAIKO_CMD,
                &RANKRANKEDSCORETAIKO_CMD,
                &PPRANKINGTAIKO_CMD,
                &RANKEDSCORERANKINGTAIKO_CMD,
                &COUNTRYRANKINGTAIKO_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!catch the beat",
            Emote::Ctb,
            vec![
                &RECENTCTB_CMD,
                &TOPCTB_CMD,
                &RECENTBESTCTB_CMD,
                &CTB_CMD,
                &OSUCOMPARECTB_CMD,
                &PPCTB_CMD,
                &WHATIFCTB_CMD,
                &RANKCTB_CMD,
                &COMMONCTB_CMD,
                &RECENTCTBLEADERBOARD_CMD,
                &RECENTCTBBELGIANLEADERBOARD_CMD,
                &OSUSTATSGLOBALSCTB_CMD,
                &OSUSTATSCOUNTCTB_CMD,
                &OSUSTATSLISTCTB_CMD,
                &SIMULATERECENTCTB_CMD,
                &RECENTLISTCTB_CMD,
                &NOCHOKESCTB_CMD,
                &MAPPERCTB_CMD,
                &TOPIFCTB_CMD,
                &TOPOLDCTB_CMD,
                &RANKRANKEDSCORECTB_CMD,
                &PPRANKINGCTB_CMD,
                &RANKEDSCORERANKINGCTB_CMD,
                &COUNTRYRANKINGCTB_CMD,
            ],
        ),
        CommandGroup::new(
            "osu!tracking",
            Emote::Tracking,
            vec![
                &TRACK_CMD,
                &TRACKMANIA_CMD,
                &TRACKTAIKO_CMD,
                &TRACKCTB_CMD,
                &TRACKLIST_CMD,
                &UNTRACK_CMD,
                &UNTRACKALL_CMD,
            ],
        ),
        CommandGroup::new(
            "twitch",
            Emote::Twitch,
            vec![&ADDSTREAM_CMD, &REMOVESTREAM_CMD, &TRACKEDSTREAMS_CMD],
        ),
        CommandGroup::new(
            "games",
            Emote::Custom("video_game"),
            vec![&MINESWEEPER_CMD, &BACKGROUNDGAME_CMD],
        ),
        CommandGroup::new(
            "utility",
            Emote::Custom("tools"),
            vec![
                &PING_CMD,
                &ROLL_CMD,
                &CONFIG_CMD,
                &COMMANDS_CMD,
                &INVITE_CMD,
                &PRUNE_CMD,
                &PREFIX_CMD,
                &AUTHORITIES_CMD,
                &ROLEASSIGN_CMD,
                &TOGGLESONGS_CMD,
            ],
        ),
        CommandGroup::new(
            "songs",
            Emote::Custom("musical_note"),
            vec![
                &BOMBSAWAY_CMD,
                &CATCHIT_CMD,
                &DING_CMD,
                &FIREANDFLAMES_CMD,
                &FIREFLIES_CMD,
                &FLAMINGO_CMD,
                &PRETENDER_CMD,
                &ROCKEFELLER_CMD,
                &SAYGOODBYE_CMD,
                &STARTAGAIN_CMD,
                &TIJDMACHINE_CMD,
            ],
        ),
        CommandGroup::new(
            "owner",
            Emote::Custom("crown"),
            vec![
                &ADDBG_CMD,
                &ADDCOUNTRY_CMD,
                &CACHE_CMD,
                &BGTAGS_CMD,
                &BGTAGSMANUAL_CMD,
                &CHANGEGAME_CMD,
                &TRACKINGTOGGLE_CMD,
                &TRACKINGSTATS_CMD,
                &TRACKINGCOOLDOWN_CMD,
                &TRACKINGINTERVAL_CMD,
            ],
        ),
    ]
}

pub struct MyCommandOption {
    pub name: &'static str,
    pub description: &'static str,
    pub help: Option<&'static str>,
    pub kind: MyCommandOptionKind,
}

pub struct MyCommandOptionBuilder {
    name: &'static str,
    description: &'static str,
    help: Option<&'static str>,
    autocomplete: bool,
    min_num: Option<Number>,
    max_num: Option<Number>,
    min_int: Option<i64>,
    max_int: Option<i64>,
}

impl MyCommandOptionBuilder {
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);

        self
    }

    pub fn autocomplete(mut self) -> Self {
        self.autocomplete = true;

        self
    }

    pub fn min_num(mut self, n: f64) -> Self {
        self.min_num = Some(Number(n));

        self
    }

    pub fn max_num(mut self, n: f64) -> Self {
        self.max_num = Some(Number(n));

        self
    }

    pub fn min_int(mut self, n: i64) -> Self {
        self.min_int = Some(n);

        self
    }

    pub fn max_int(mut self, n: i64) -> Self {
        self.max_int = Some(n);

        self
    }

    pub fn subcommand(self, options: Vec<MyCommandOption>) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::SubCommand { options },
        }
    }

    pub fn subcommandgroup(self, options: Vec<MyCommandOption>) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::SubCommandGroup { options },
        }
    }

    pub fn string(self, choices: Vec<CommandOptionChoice>, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::String {
                autocomplete: self.autocomplete,
                choices,
                required,
            },
        }
    }

    pub fn integer(self, choices: Vec<CommandOptionChoice>, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Integer {
                choices,
                required,
                min: self.min_int,
                max: self.max_int,
            },
        }
    }

    pub fn number(self, choices: Vec<CommandOptionChoice>, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Number {
                choices,
                required,
                min: self.min_num,
                max: self.max_num,
            },
        }
    }

    pub fn boolean(self, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Boolean { required },
        }
    }

    pub fn user(self, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::User { required },
        }
    }

    pub fn channel(self, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Channel { required },
        }
    }

    pub fn role(self, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Role { required },
        }
    }

    pub fn mentionable(self, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Mentionable { required },
        }
    }
}

impl MyCommandOption {
    pub fn builder(name: &'static str, description: &'static str) -> MyCommandOptionBuilder {
        MyCommandOptionBuilder {
            name,
            description,
            help: None,
            autocomplete: false,
            min_num: None,
            max_num: None,
            min_int: None,
            max_int: None,
        }
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);

        self
    }
}

pub enum MyCommandOptionKind {
    SubCommand {
        options: Vec<MyCommandOption>,
    },
    SubCommandGroup {
        options: Vec<MyCommandOption>,
    },
    String {
        autocomplete: bool,
        choices: Vec<CommandOptionChoice>,
        required: bool,
    },
    Integer {
        choices: Vec<CommandOptionChoice>,
        required: bool,
        min: Option<i64>,
        max: Option<i64>,
    },
    Number {
        choices: Vec<CommandOptionChoice>,
        required: bool,
        min: Option<Number>,
        max: Option<Number>,
    },
    Boolean {
        required: bool,
    },
    User {
        required: bool,
    },
    Channel {
        required: bool,
    },
    Role {
        required: bool,
    },
    Mentionable {
        required: bool,
    },
}

impl From<MyCommandOption> for CommandOption {
    fn from(option: MyCommandOption) -> Self {
        match option.kind {
            MyCommandOptionKind::SubCommand { options } => {
                let options = options.into_iter().map(Into::into).collect();

                let inner = OptionsCommandOptionData {
                    options,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                };

                Self::SubCommand(inner)
            }
            MyCommandOptionKind::SubCommandGroup { options } => {
                let options = options.into_iter().map(Into::into).collect();

                let inner = OptionsCommandOptionData {
                    options,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                };

                Self::SubCommandGroup(inner)
            }
            MyCommandOptionKind::String {
                autocomplete,
                choices,
                required,
            } => {
                let inner = ChoiceCommandOptionData {
                    autocomplete,
                    choices,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::String(inner)
            }
            MyCommandOptionKind::Integer {
                choices,
                required,
                min,
                max,
            } => {
                let inner = NumberCommandOptionData {
                    autocomplete: false,
                    choices,
                    description: option.description.to_owned(),
                    max_value: max.map(CommandOptionValue::Integer),
                    min_value: min.map(CommandOptionValue::Integer),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Integer(inner)
            }
            MyCommandOptionKind::Number {
                choices,
                required,
                min,
                max,
            } => {
                let inner = NumberCommandOptionData {
                    autocomplete: false,
                    choices,
                    description: option.description.to_owned(),
                    max_value: max.map(CommandOptionValue::Number),
                    min_value: min.map(CommandOptionValue::Number),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Number(inner)
            }
            MyCommandOptionKind::Boolean { required } => {
                let inner = BaseCommandOptionData {
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Boolean(inner)
            }
            MyCommandOptionKind::User { required } => {
                let inner = BaseCommandOptionData {
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::User(inner)
            }
            MyCommandOptionKind::Channel { required } => {
                let inner = ChannelCommandOptionData {
                    channel_types: Vec::new(), // TODO: Make customizable
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Channel(inner)
            }
            MyCommandOptionKind::Role { required } => {
                let inner = BaseCommandOptionData {
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Role(inner)
            }
            MyCommandOptionKind::Mentionable { required } => {
                let inner = BaseCommandOptionData {
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Mentionable(inner)
            }
        }
    }
}

pub struct MyCommand {
    name: &'static str,
    description: &'static str,
    help: Option<&'static str>,
    authority: bool,
    options: Vec<MyCommandOption>,
}

impl MyCommand {
    pub fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            help: None,
            authority: false,
            options: Vec::new(),
        }
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);

        self
    }

    pub fn authority(mut self) -> Self {
        self.authority = true;

        self
    }

    pub fn options(mut self, options: Vec<MyCommandOption>) -> Self {
        self.options = options;

        self
    }
}

impl From<MyCommand> for Command {
    fn from(command: MyCommand) -> Self {
        Self {
            application_id: None,
            guild_id: None,
            name: command.name.to_owned(),
            default_permission: None,
            description: command.description.to_owned(),
            id: None,
            kind: CommandType::ChatInput,
            options: command.options.into_iter().map(Into::into).collect(),
            version: Id::new(1),
        }
    }
}

// BTreeMap to have a stable order
pub struct SlashCommands(Trie<&'static str, fn() -> MyCommand>);

type CommandKeys<'t> = Copied<Keys<'t, &'static str, fn() -> MyCommand>>;

impl SlashCommands {
    pub fn command(&self, command: &str) -> Option<MyCommand> {
        self.0.get(command).map(|f| (f)())
    }

    pub fn collect(&self) -> Vec<Command> {
        self.0.values().map(|f| (f)().into()).collect()
    }

    pub fn names(&self) -> CommandKeys<'_> {
        self.0.keys().copied()
    }

    pub fn descendants(&self, prefix: &str) -> Option<CommandKeys<'_>> {
        self.0
            .get_raw_descendant(prefix)
            .map(|sub| sub.keys().copied())
    }
}

lazy_static! {
    pub static ref SLASH_COMMANDS: SlashCommands = {
        let mut trie = Trie::new();
        let mut insert = |name: &'static str, f: fn() -> MyCommand| trie.insert(name, f);

        insert(HELP, help::define_help);
        insert("recent", define_recent);
        insert("rs", define_rs);
        insert("rb", define_rb);
        insert("track", define_track);
        insert("owner", define_owner);
        insert("song", define_song);
        insert("trackstream", define_trackstream);
        insert("minesweeper", define_minesweeper);
        insert("invite", define_invite);
        insert("roll", define_roll);
        insert("commands", define_commands);
        insert("prune", define_prune);
        insert("config", define_config);
        insert("roleassign", define_roleassign);
        insert("ping", define_ping);
        insert("ratios", define_ratios);
        insert("mostplayed", define_mostplayed);
        insert("matchcost", define_matchcost);
        insert("link", define_link);
        insert("bws", define_bws);
        insert("avatar", define_avatar);
        insert("map", define_map);
        insert("matchlive", define_matchlive);
        insert("fix", define_fix);
        insert("whatif", define_whatif);
        insert("simulate", define_simulate);
        insert("search", define_mapsearch);
        insert("leaderboard", define_leaderboard);
        insert(PROFILE, define_profile);
        insert("rank", define_rank);
        insert("pp", define_pp);
        insert("compare", define_compare);
        insert("cs", define_cs);
        insert("medal", define_medal);
        insert("osekai", define_osekai);
        insert("ranking", define_ranking);
        insert("osustats", define_osustats);
        insert("osc", define_osc);
        insert("top", define_top);
        insert("pinned", define_pinned);
        insert("topif", define_topif);
        insert("mapper", define_mapper);
        insert("nochoke", define_nochoke);
        insert("topold", define_topold);
        insert("snipe", define_snipe);
        insert("serverconfig", define_serverconfig);
        insert("serverleaderboard", define_serverleaderboard);
        insert("bg", define_bg);

        SlashCommands(trie)
    };
}
