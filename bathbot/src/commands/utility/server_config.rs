use bathbot_macros::{SlashCommand, command};
use bathbot_model::command_fields::{EnableDisable, ShowHideOption};
use bathbot_psql::model::configs::{GuildConfig, HideSolutions, ListSize, Retries, ScoreData};
use bathbot_util::constants::GENERAL_ISSUE;
use eyre::{Report, Result};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    guild::Permissions,
    id::{Id, marker::RoleMarker},
};

use super::AuthorityCommandKind;
use crate::{
    Context,
    core::commands::CommandOrigin,
    embeds::{EmbedData, ServerConfigEmbed},
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "serverconfig",
    dm_permission = false,
    desc = "Adjust configurations or authority roles for this server"
)]
#[flags(AUTHORITY, SKIP_DEFER, ONLY_GUILDS)]
pub enum ServerConfig {
    #[command(name = "authorities")]
    Authorities(ServerConfigAuthorities),
    #[command(name = "edit")]
    Edit(ServerConfigEdit),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "authorities",
    desc = "Adjust authority roles for a server",
    help = "To use certain commands, users require a special status.\n\
    This command adjusts the authority status of roles.\n\
    Any member with an authority role can use these higher commands.\n\n\
    Authority commands: `matchlive`, `prune`, `roleassign`, \
    `serverconfig`, `track`, `trackstream`."
)]
pub enum ServerConfigAuthorities {
    #[command(name = "add")]
    Add(ServerConfigAuthoritiesAdd),
    #[command(name = "remove")]
    Remove(ServerConfigAuthoritiesRemove),
    #[command(name = "remove_all")]
    RemoveAll(ServerConfigAuthoritiesRemoveAll),
    #[command(name = "list")]
    List(ServerConfigAuthoritiesList),
}

impl From<ServerConfigAuthorities> for AuthorityCommandKind {
    #[inline]
    fn from(args: ServerConfigAuthorities) -> Self {
        match args {
            ServerConfigAuthorities::Add(args) => Self::Add(args.role),
            ServerConfigAuthorities::Remove(args) => Self::Remove(args.role),
            ServerConfigAuthorities::RemoveAll(_) => Self::RemoveAll,
            ServerConfigAuthorities::List(_) => Self::List,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "add",
    desc = "Add authority status to a role",
    help = "Add authority status to a role.\n\
    Servers can have at most 10 authority roles."
)]
pub struct ServerConfigAuthoritiesAdd {
    #[command(desc = "Specify the role that should gain authority status")]
    role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "remove",
    desc = "Remove authority status from a role",
    help = "Remove authority status from a role.\n\
    You can only use this if the removed role would __not__ make you lose authority status yourself."
)]
pub struct ServerConfigAuthoritiesRemove {
    #[command(desc = "Specify the role that should gain authority status")]
    role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "remove_all",
    desc = "Remove authority status from all roles",
    help = "Remove authority status from all roles.\n\
    You can only use this if you have the admin permission."
)]
pub struct ServerConfigAuthoritiesRemoveAll;

#[derive(CommandModel, CreateCommand)]
#[command(name = "list", desc = "Display all current authority roles")]
pub struct ServerConfigAuthoritiesList;

#[derive(CommandModel, CreateCommand, Default)]
#[command(name = "edit", desc = "Adjust configurations for a server")]
pub struct ServerConfigEdit {
    #[command(desc = "Choose whether song commands can be used or not")]
    song_commands: Option<EnableDisable>,
    #[command(
        desc = "Adjust the amount of scores shown per page in top, rb, pinned, ...",
        help = "Adjust the amount of scores shown per page in top, rb, pinned, and mapper.\n\
        `Condensed` shows 10 scores, `Detailed` shows 5, and `Single` shows 1.\n\
        Applies only if the member has not specified a config for themselves."
    )]
    list_embeds: Option<ListSize>,
    #[command(
        desc = "Should the amount of retries be shown for the recent command?",
        help = "Should the amount of retries be shown for the `recent` command?\n\
        Applies only if the member has not specified a config for themselves."
    )]
    retries: Option<Retries>,
    #[command(
        desc = "Should the recent command include a render button?",
        help = "Should the `recent` command include a render button?\n\
        The button would be a shortcut for the `/render` command.\n\
        If hidden, the button will never show. If shown, members \
        will have the option to choose via `/config`."
    )]
    render_button: Option<ShowHideOption>,
    #[command(
        desc = "Are members allowed to use custom skins when rendering?",
        help = "Are members allowed to use custom skins when rendering?\n\
        Handy for disallowing potentially obscene skins."
    )]
    allow_custom_skins: Option<bool>,
    #[command(desc = "Should medal solutions should be hidden behind spoiler tags?")]
    hide_medal_solutions: Option<HideSolutions>,
    #[command(
        desc = "Whether scores should be requested as lazer or stable scores",
        help = "Whether scores should be requested as lazer or stable scores.\n\
        They have a different score and grade calculation and only lazer adds the new mods.\n\
        Applies only if the member has not specified a config for themselves."
    )]
    score_data: Option<ScoreData>,
}

impl ServerConfigEdit {
    fn any(&self) -> bool {
        let Self {
            song_commands,
            list_embeds,
            retries,
            render_button,
            allow_custom_skins,
            hide_medal_solutions,
            score_data,
        } = self;

        song_commands.is_some()
            || list_embeds.is_some()
            || retries.is_some()
            || render_button.is_some()
            || allow_custom_skins.is_some()
            || hide_medal_solutions.is_some()
            || score_data.is_some()
    }
}

async fn slash_serverconfig(mut command: InteractionCommand) -> Result<()> {
    let args = ServerConfig::from_interaction(command.input_data())?;

    serverconfig((&mut command).into(), args).await
}

#[command]
#[desc("Check the current configurations for a server")]
#[help(
    "Check the current configurations for a server.\n\
    Use `/serverconfig edit` to edit them."
)]
#[flags(SKIP_DEFER, ONLY_GUILDS)]
#[group(Utility)]
async fn prefix_serverconfig(msg: &Message, _: Args<'_>, perms: Option<Permissions>) -> Result<()> {
    let args = ServerConfig::Edit(ServerConfigEdit::default());
    let orig = CommandOrigin::from_msg(msg, perms);

    serverconfig(orig, args).await
}

async fn serverconfig(orig: CommandOrigin<'_>, args: ServerConfig) -> Result<()> {
    let guild_id = orig.guild_id().unwrap();

    let guild = match Context::cache().guild(guild_id).await {
        Ok(Some(guild)) => guild,
        Ok(None) => {
            warn!("Missing guild {guild_id} in cache");
            orig.error(GENERAL_ISSUE).await?;

            return Ok(());
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(Report::new(err));
        }
    };

    let args = match args {
        ServerConfig::Authorities(args) => {
            return super::authorities(orig, args.into()).await;
        }
        ServerConfig::Edit(edit) => edit,
    };

    if args.any() {
        let f = |config: &mut GuildConfig| {
            let ServerConfigEdit {
                list_embeds,
                retries,
                song_commands,
                render_button,
                allow_custom_skins,
                hide_medal_solutions,
                score_data,
            } = args;

            if let Some(list_embeds) = list_embeds {
                config.list_size = Some(list_embeds);
            }

            if let Some(retries) = retries {
                config.retries = Some(retries);
            }

            if let Some(with_lyrics) = song_commands {
                config.allow_songs = Some(with_lyrics == EnableDisable::Enable);
            }

            if let Some(render_button) = render_button {
                config.render_button = Some(render_button == ShowHideOption::Show);
            }

            if let Some(allow_custom_skins) = allow_custom_skins {
                config.allow_custom_skins = Some(allow_custom_skins);
            }

            if let Some(hide_medal_solutions) = hide_medal_solutions {
                config.hide_medal_solution = Some(hide_medal_solutions);
            }

            if let Some(score_data) = score_data {
                config.score_data = Some(score_data);
            }
        };

        if let Err(err) = Context::guild_config().update(guild_id, f).await {
            let _ = orig.error_callback(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to update guild config"));
        }
    }

    let config = Context::guild_config()
        .peek(guild_id, GuildConfig::to_owned)
        .await;

    let mut authorities = Vec::with_capacity(config.authorities.len());

    for &role in config.authorities.iter() {
        if let Ok(Some(role)) = Context::cache().role(guild_id, role).await {
            authorities.push(role.name.as_ref().to_owned());
        }
    }

    let embed = ServerConfigEmbed::new(guild, config, &authorities);
    let builder = embed.build().into();
    orig.callback(builder).await?;

    Ok(())
}
