#![cfg(feature = "osutracking")]

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use command_macros::SlashCommand;
use eyre::Result;
use rosu_v2::prelude::{GameMode, OsuError, Username};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::commands::prefix::Args,
    manager::redis::osu::UserArgs,
    util::{interaction::InteractionCommand, CowUtils, InteractionCommandExt},
    Context,
};

pub use self::{track::*, track_list::*, untrack::*, untrack_all::*};

use super::GameModeOption;

mod track;
mod track_list;
mod untrack;
mod untrack_all;

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "track")]
#[flags(AUTHORITY, ONLY_GUILDS)]
/// Track top score updates for players
pub enum Track {
    #[command(name = "add")]
    Add(TrackAdd),
    #[command(name = "remove")]
    Remove(TrackRemove),
    #[command(name = "list")]
    List(TrackList),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "add",
    help = "Add users to the tracking list for this channel.\n\
    If a tracked user gets a new top score, this channel will be notified about it."
)]
/// Track top scores of a player
pub struct TrackAdd {
    /// Choose a username to be tracked
    name: String,
    /// Specify a mode for the tracked users
    mode: GameModeOption,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "If not specified, updates in the user's top50 will trigger notification messages.\n\
        Instead of the top50, this `limit` option allows to adjust the maximum index within \
        the top scores.\nThe value must be between 1 and 100."
    )]
    /// Between 1-100, default 50, notify on updates of the user's top X scores
    limit: Option<u8>,
    /// Specify a second username
    name2: Option<String>,
    /// Specify a third username
    name3: Option<String>,
    /// Specify a fourth username
    name4: Option<String>,
    /// Specify a fifth username
    name5: Option<String>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "remove",
    help = "Untrack players in a channel i.e. stop sending notifications when they get new top scores"
)]
/// Untrack players in a channel
pub enum TrackRemove {
    #[command(name = "user")]
    User(TrackRemoveUser),
    #[command(name = "all")]
    All(TrackRemoveAll),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "user")]
/// Untrack specific users in a channel
pub struct TrackRemoveUser {
    /// Choose a username to be untracked
    name: String,
    /// Specify a mode for the tracked users
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "all")]
/// Untrack all users in a channel
pub struct TrackRemoveAll {
    /// Specify a mode for the tracked users
    mode: Option<GameModeOption>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "list")]
/// List all players that are tracked in this channel
pub struct TrackList;

async fn slash_track(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Track::from_interaction(command.input_data())? {
        Track::Add(add) => track(ctx, (&mut command).into(), add.into()).await,
        Track::Remove(TrackRemove::User(user)) => {
            untrack(ctx, (&mut command).into(), user.into()).await
        }
        Track::Remove(TrackRemove::All(all)) => {
            untrackall(ctx, (&mut command).into(), all.mode.map(GameMode::from)).await
        }
        Track::List(_) => tracklist(ctx, (&mut command).into()).await,
    }
}

async fn get_names<'n>(
    ctx: &Context,
    names: &'n [String],
    mode: GameMode,
) -> Result<HashMap<Username, u32>, (OsuError, Cow<'n, str>)> {
    let mut entries = match ctx.osu_user().ids(names).await {
        Ok(names) => names,
        Err(err) => {
            warn!("{:?}", err.wrap_err("failed to get user ids by names"));

            HashMap::new()
        }
    };

    if entries.len() != names.len() {
        for name in names {
            let name = name.cow_to_ascii_lowercase();

            if entries.keys().all(|n| name != n.cow_to_ascii_lowercase()) {
                let args = UserArgs::username(ctx, name.as_ref()).await.mode(mode);

                match ctx.redis().osu_user(args).await {
                    Ok(user) => entries.insert(user.username().into(), user.user_id()),
                    Err(err) => return Err((err, name)),
                };
            }
        }
    }

    Ok(entries)
}

struct TrackArgs {
    mode: Option<GameMode>,
    name: String,
    limit: Option<u8>,
    more_names: Vec<String>,
}

impl TrackArgs {
    async fn args(mode: Option<GameMode>, args: Args<'_>) -> Result<Self, Cow<'static, str>> {
        let mut name = None;
        let mut more_names = Vec::new();
        let mut limit = args.num.map(|n| n.min(100) as u8);

        for arg in args.map(CowUtils::cow_to_ascii_lowercase) {
            if let Some(idx) = arg.find('=').filter(|&i| i > 0) {
                let key = &arg[..idx];
                let value = arg[idx + 1..].trim_end();

                match key {
                    "limit" | "l" => match value.parse() {
                        Ok(num) => limit = Some(num),
                        Err(_) => {
                            let content = "Failed to parse `limit`. Must be either an integer.";

                            return Err(content.into());
                        }
                    },
                    _ => {
                        let content = format!(
                            "Unrecognized option `{key}`.\nAvailable options are: `limit`."
                        );

                        return Err(content.into());
                    }
                }
            } else if name.is_none() {
                name = Some(arg.into_owned());
            } else if more_names.len() < 9 {
                more_names.push(arg.into_owned());
            }
        }

        let name = match name {
            Some(name) => name,
            None => return Err("You must specify at least one username".into()),
        };

        let args = Self {
            name,
            limit,
            more_names,
            mode,
        };

        Ok(args)
    }
}

impl From<TrackAdd> for TrackArgs {
    fn from(add: TrackAdd) -> Self {
        let TrackAdd {
            name,
            mode,
            limit,
            name2,
            name3,
            name4,
            name5,
        } = add;

        let mut more_names = Vec::new();

        if let Some(name) = name2 {
            more_names.push(name);
        }

        if let Some(name) = name3 {
            more_names.push(name);
        }

        if let Some(name) = name4 {
            more_names.push(name);
        }

        if let Some(name) = name5 {
            more_names.push(name);
        }

        Self {
            mode: Some(mode.into()),
            name,
            limit,
            more_names,
        }
    }
}

impl From<TrackRemoveUser> for TrackArgs {
    fn from(remove: TrackRemoveUser) -> Self {
        let TrackRemoveUser { name, mode } = remove;

        Self {
            mode: mode.map(GameMode::from),
            name,
            limit: None,
            more_names: Vec::new(),
        }
    }
}
