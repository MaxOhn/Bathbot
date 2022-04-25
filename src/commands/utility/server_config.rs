use std::sync::Arc;

use command_macros::{command, SlashCommand};
use twilight_cache_inmemory::model::CachedGuild;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{
        marker::{GuildMarker, RoleMarker},
        Id,
    },
    util::ImageHash,
};

use crate::{
    commands::{osu::ProfileSize, EnableDisable, ShowHideOption},
    database::GuildConfig,
    embeds::{EmbedData, ServerConfigEmbed},
    util::{constants::GENERAL_ISSUE, ApplicationCommandExt},
    BotResult, Context,
};

use super::{AuthorityCommandKind, ConfigEmbeds, ConfigMinimizedPp};

pub struct GuildData {
    pub icon: Option<ImageHash>,
    pub id: Id<GuildMarker>,
    pub name: String,
}

impl From<&CachedGuild> for GuildData {
    fn from(guild: &CachedGuild) -> Self {
        Self {
            icon: guild.icon().map(ImageHash::to_owned),
            id: guild.id(),
            name: guild.name().to_owned(),
        }
    }
}

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "serverconfig")]
#[flags(AUTHORITY, ONLY_GUILDS, SKIP_DEFER)]
/// Adjust configurations or authority roles for this server
pub enum ServerConfig {
    #[command(name = "authorities")]
    Authorities(ServerConfigAuthorities),
    #[command(name = "edit")]
    Edit(ServerConfigEdit),
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "authorities",
    help = "To use certain commands, users require a special status.\n\
    This command adjusts the authority status of roles.\n\
    Any member with an authority role can use these higher commands.\n\n\
    Authority commands: `matchlive`, `prune`, `roleassign`, \
    `serverconfig`, `track`, `trackstream`."
)]
/// Adjust authority roles for a server
pub enum ServerConfigAuthorities {
    #[command(name = "add")]
    Add(ServerConfigAuthoritiesAdd),
    #[command(name = "remove")]
    Remove(ServerConfigAuthoritiesRemove),
    #[command(name = "list")]
    List(ServerConfigAuthoritiesList),
}

impl From<ServerConfigAuthorities> for AuthorityCommandKind {
    fn from(args: ServerConfigAuthorities) -> Self {
        match args {
            ServerConfigAuthorities::Add(args) => Self::Add(args.role.get()),
            ServerConfigAuthorities::Remove(args) => Self::Remove(args.role.get()),
            ServerConfigAuthorities::List(_) => Self::List,
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "add",
    help = "Add authority status to a role.\n\
    Servers can have at most 10 authority roles."
)]
/// Add authority status to a role
pub struct ServerConfigAuthoritiesAdd {
    /// Specify the role that should gain authority status
    role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(
    name = "remove",
    help = "Remove authority status from a role.\n\
    You can only use this if the removed role would __not__ make you lose authority status yourself."
)]
/// Remove authority status from a role
pub struct ServerConfigAuthoritiesRemove {
    /// Specify the role that should gain authority status
    role: Id<RoleMarker>,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "list")]
/// Display all current authority roles
pub struct ServerConfigAuthoritiesList;

#[derive(CommandModel, CreateCommand)]
#[command(name = "edit")]
/// Adjust configurations for a server
pub struct ServerConfigEdit {
    /// Choose whether song commands can be used or not
    song_commands: Option<EnableDisable>,
    #[command(help = "What initial size should the profile command be?\n\
        Applies only if the member has not specified a config for themselves.")]
    /// What initial size should the profile command be?
    profile: Option<ProfileSize>,
    #[command(help = "Some embeds are pretty chunky and show too much data.\n\
        With this option you can make those embeds minimized by default.\n\
        Affected commands are: `compare score`, `recent score`, `recent simulate`, \
        and any command showing top scores when the `index` option is specified.\n\
        Applies only if the member has not specified a config for themselves.")]
    /// What size should the recent, compare, simulate, ... commands be?
    embeds: Option<ConfigEmbeds>,
    #[command(
        help = "Should the amount of retries be shown for the `recent` command?\n\
        Applies only if the member has not specified a config for themselves."
    )]
    /// Should the amount of retries be shown for the recent command?
    retries: Option<ShowHideOption>,
    #[command(
        min_value = 1,
        max_value = 100,
        help = "Specify the default track limit for tracking user's osu! top scores.\n\
        The value must be between 1 and 100, defaults to 50."
    )]
    /// Specify the default track limit for osu! top scores
    track_limit: Option<i64>,
    /// Specify whether the recent command should show max or if-fc pp when minimized
    minimized_pp: Option<ConfigMinimizedPp>,
}

impl ServerConfigEdit {
    fn any(&self) -> bool {
        self.song_commands.is_some()
            || self.profile.is_some()
            || self.embeds.is_some()
            || self.retries.is_some()
            || self.track_limit.is_some()
            || self.minimized_pp.is_some()
    }
}

async fn slash_serverconfig(
    ctx: Arc<Context>,
    mut command: Box<ApplicationCommand>,
) -> BotResult<()> {
    let args = ServerConfig::from_interaction(command.input_data())?;

    let guild_id = command.guild_id.unwrap();

    let guild = match ctx.cache.guild(guild_id, |guild| guild.into()) {
        Ok(guild) => guild,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let args = match args {
        ServerConfig::Authorities(args) => {
            return super::authorities(ctx, command.into(), args.into()).await
        }
        ServerConfig::Edit(edit) => edit,
    };

    if args.any() {
        let f = |config: &mut GuildConfig| {
            let ServerConfigEdit {
                embeds,
                minimized_pp,
                profile,
                retries,
                song_commands,
                track_limit,
            } = args;

            if let Some(embeds) = embeds {
                config.embeds_size = Some(embeds.into());
            }

            if let Some(pp) = minimized_pp {
                config.minimized_pp = Some(pp.into());
            }

            if let Some(profile) = profile {
                config.profile_size = Some(profile);
            }

            if let Some(retries) = retries {
                config.show_retries = Some(retries == ShowHideOption::Show);
            }

            if let Some(limit) = track_limit {
                config.track_limit = Some(limit as u8);
            }

            if let Some(with_lyrics) = song_commands {
                config.with_lyrics = Some(with_lyrics == EnableDisable::Enable);
            }
        };

        if let Err(err) = ctx.update_guild_config(guild_id, f).await {
            let _ = command.error_callback(&ctx, GENERAL_ISSUE).await;

            return Err(err);
        }
    }

    let config = ctx.guild_config(guild_id).await;
    let mut authorities = Vec::with_capacity(config.authorities.len());

    for &auth in &config.authorities {
        if let Some(Ok(name)) =
            Id::new_checked(auth).map(|role| ctx.cache.role(role, |role| role.name.to_owned()))
        {
            authorities.push(name);
        }
    }

    let embed = ServerConfigEmbed::new(guild, config, &authorities);
    let builder = embed.build().into();
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
