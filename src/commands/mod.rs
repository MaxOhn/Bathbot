/// E.g: `bail_cmd_option!("link", subcommand, name)`
macro_rules! bail_cmd_option {
    ($cmd:expr, string, $name:ident) => {
        bail_cmd_option!(@ $cmd, "string", $name)
    };
    ($cmd:expr, integer, $name:ident) => {
        bail_cmd_option!(@ $cmd, "integer", $name)
    };
    ($cmd:expr, boolean, $name:ident) => {
        bail_cmd_option!(@ $cmd, "boolean", $name)
    };
    ($cmd:expr, subcommand, $name:ident) => {
        bail_cmd_option!(@ $cmd, "subcommand", $name)
    };
    ($cmd:expr, $any:tt, $name:ident) => {
       compile_error!("expected `string`, `integer`, `boolean`, or `subcommand` as second argument")
    };

    (@ $cmd:expr, $kind:literal, $name:ident) => {
        return Err(crate::Error::UnexpectedCommandOption {
            cmd: $cmd,
            kind: $kind,
            name: $name,
        })
    };
}

/// E.g: `parse_mode_option!(value, "recent score")`
macro_rules! parse_mode_option {
    ($value:ident, $location:literal) => {
        match $value.as_str() {
            crate::util::constants::common_literals::OSU => Some(GameMode::STD),
            crate::util::constants::common_literals::TAIKO => Some(GameMode::TKO),
            crate::util::constants::common_literals::CTB => Some(GameMode::CTB),
            crate::util::constants::common_literals::MANIA => Some(GameMode::MNA),
            _ => bail_cmd_option!(concat!($location, " mode"), string, $value),
        }
    };
}

/// E.g: `parse_discord_option!(ctx, value, "top rebalance")`
macro_rules! parse_discord_option {
    ($ctx:ident, $value:ident, $location:literal) => {
        match $value.parse() {
            Ok(id) => match $ctx
                .psql()
                .get_user_osu(twilight_model::id::UserId(id))
                .await?
            {
                Some(osu) => osu,
                None => {
                    let content = format!("<@{}> is not linked to an osu profile", id);

                    return Ok(Err(content.into()));
                }
            },
            Err(_) => bail_cmd_option!(concat!($location, " discord"), string, $value),
        }
    };
}

pub mod fun;
pub mod help;
pub mod osu;
pub mod owner;
pub mod songs;
pub mod tracking;
pub mod twitch;
pub mod utility;

use fun::*;
use osu::*;
use owner::*;
use songs::*;
use tracking::*;
use twitch::*;
use utility::*;

use std::collections::BTreeMap;

use twilight_model::application::command::{
    BaseCommandOptionData, ChoiceCommandOptionData, Command, CommandOption, CommandOptionChoice,
    CommandType, OptionsCommandOptionData,
};

use crate::{
    core::CommandGroup,
    util::{
        constants::common_literals::{HELP, PROFILE},
        Emote,
    },
};

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
                &REBALANCE_CMD,
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
                &ECHO_CMD,
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
}

impl MyCommandOptionBuilder {
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);

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
            kind: MyCommandOptionKind::String { choices, required },
        }
    }

    pub fn integer(self, choices: Vec<CommandOptionChoice>, required: bool) -> MyCommandOption {
        MyCommandOption {
            name: self.name,
            description: self.description,
            help: self.help,
            kind: MyCommandOptionKind::Integer { choices, required },
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
        choices: Vec<CommandOptionChoice>,
        required: bool,
    },
    Integer {
        choices: Vec<CommandOptionChoice>,
        required: bool,
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
                    required: false,
                };

                Self::SubCommand(inner)
            }
            MyCommandOptionKind::SubCommandGroup { options } => {
                let options = options.into_iter().map(Into::into).collect();

                let inner = OptionsCommandOptionData {
                    options,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required: false,
                };

                Self::SubCommandGroup(inner)
            }
            MyCommandOptionKind::String { choices, required } => {
                let inner = ChoiceCommandOptionData {
                    choices,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::String(inner)
            }
            MyCommandOptionKind::Integer { choices, required } => {
                let inner = ChoiceCommandOptionData {
                    choices,
                    description: option.description.to_owned(),
                    name: option.name.to_owned(),
                    required,
                };

                Self::Integer(inner)
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
                let inner = BaseCommandOptionData {
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
        }
    }
}

// BTreeMap to have a stable order
pub struct SlashCommands(BTreeMap<&'static str, fn() -> MyCommand>);

impl SlashCommands {
    pub fn command(&self, command: &str) -> Option<MyCommand> {
        self.0.get(command).map(|f| (f)())
    }

    pub fn collect(&self) -> Vec<Command> {
        self.0.values().map(|f| (f)().into()).collect()
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.keys().copied()
    }
}

lazy_static! {
    pub static ref SLASH_COMMANDS: SlashCommands = {
        let mut map = BTreeMap::new();
        let mut insert = |name: &'static str, f: fn() -> MyCommand| map.insert(name, f);

        insert(HELP, help::define_help);
        insert("recent", define_recent);
        insert("track", define_track);
        insert("owner", define_owner);
        insert("song", define_song);
        insert("trackstream", define_trackstream);
        insert("minesweeper", define_minesweeper);
        insert("invite", define_invite);
        insert("roll", define_roll);
        insert("togglesongs", define_togglesongs);
        insert("commands", define_commands);
        insert("authorities", define_authorities);
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
        insert("reach", define_reach);
        insert("compare", define_compare);
        insert("medal", define_medal);
        insert("osekai", define_osekai);
        insert("ranking", define_ranking);
        insert("osustats", define_osustats);
        insert("top", define_top);
        insert("snipe", define_snipe);

        SlashCommands(map)
    };
}
