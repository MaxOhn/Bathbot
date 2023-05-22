use std::sync::Arc;

use bathbot_macros::{HasName, SlashCommand};
use bathbot_util::{constants::GENERAL_ISSUE, matcher, MessageBuilder};
use eyre::{Report, Result, WrapErr};
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::id::{marker::UserMarker, Id};

use crate::{
    active::{self, ActiveMessages},
    core::{commands::CommandOrigin, Context},
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "skin", desc = "Set your own skin or check someone else's")]
pub enum Skin {
    #[command(name = "check")]
    Check(CheckSkin),
    #[command(name = "all")]
    All(AllSkin),
    #[command(name = "set")]
    Set(SetSkin),
    #[command(name = "unset")]
    Unset(UnsetSkin),
}

pub async fn slash_skin(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    match Skin::from_interaction(command.input_data())? {
        Skin::Check(args) => args.process(&ctx, &command).await,
        Skin::All(args) => args.process(ctx, &mut command).await,
        Skin::Set(args) => args.process(&ctx, &command).await,
        Skin::Unset(args) => args.process(&ctx, &command).await,
    }
}

#[derive(CommandModel, CreateCommand, HasName)]
#[command(name = "check", desc = "Check someone's skin")]
pub struct CheckSkin {
    #[command(desc = "Specify a username")]
    name: Option<String>,
    #[command(
        desc = "Specify a linked discord user",
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    discord: Option<Id<UserMarker>>,
}

impl CheckSkin {
    async fn process(self, ctx: &Context, command: &InteractionCommand) -> Result<()> {
        if let Some(username) = self.name {
            // User specified an osu! username
            match ctx.user_config().skin_from_osu_name(&username).await {
                Ok(Some(skin_url)) => {
                    let content = format!("`{username}`'s current skin: {skin_url}");
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Ok(None) => {
                    let content = format!("`{username}` has not yet set their skin.");
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Err(err) => {
                    let _ = command.error(ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }
        } else if let Some(user_id) = self.discord {
            // User specified a discord user
            match ctx.user_config().skin(user_id).await {
                Ok(Some(skin_url)) => {
                    let content = format!("<@{user_id}>'s current skin: {skin_url}");
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Ok(None) => {
                    let content = format!("<@{user_id}> has not yet set their skin.");
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Err(err) => {
                    let _ = command.error(ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }
        } else {
            // User didn't specify anything, choose user themselves
            match ctx.user_config().skin(command.user_id()?).await {
                Ok(Some(skin_url)) => {
                    let content = format!("Your current skin: {skin_url}");
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Ok(None) => {
                    let content = "You have not yet set your skin. You can do so with `/skin set`";
                    let builder = MessageBuilder::new().embed(content);
                    command.update(ctx, &builder).await?;
                }
                Err(err) => {
                    let _ = command.error(ctx, GENERAL_ISSUE).await;

                    return Err(err);
                }
            }
        }

        Ok(())
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "all", desc = "List skins of all users")]
pub struct AllSkin;

impl AllSkin {
    async fn process(self, ctx: Arc<Context>, command: &mut InteractionCommand) -> Result<()> {
        match ctx.user_config().all_skins().await {
            Ok(entries) => {
                let pagination = active::impls::SkinsPagination::builder()
                    .entries(entries.into_boxed_slice())
                    .msg_owner(command.user_id()?)
                    .build();

                ActiveMessages::builder(pagination)
                    .start_by_update(true)
                    .begin(ctx, CommandOrigin::from(command))
                    .await
                    .wrap_err("Failed to begin active message")
            }
            Err(err) => {
                let _ = command.error(&ctx, GENERAL_ISSUE).await;

                Err(err)
            }
        }
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "set", desc = "Set the skin you use")]
pub struct SetSkin {
    #[command(
        desc = "Specify a download link for your skin",
        help = "Specify a download link for your skin.\n\
        Must be a URL to a direct-download of an .osk file or of one of these approved sites:\n\
        - `https://drive.google.com`\n\
        - `https://www.dropbox.com`\n\
        - `https://mega.nz`\n\
        - `https://www.mediafire.com`\n\
        - `https://skins.osuck.net`\n\
        - `https://github.com`\n\
        If you want to suggest another site let Badewanne3 know."
    )]
    url: String,
}

impl SetSkin {
    async fn process(self, ctx: &Context, command: &InteractionCommand) -> Result<()> {
        let Self { url } = self;

        match SkinValidation::check(ctx, command, &url).await? {
            ValidationStatus::Continue => {}
            ValidationStatus::Handled => return Ok(()),
        }

        let update_fut = ctx
            .user_config()
            .update_skin(command.user_id()?, Some(&url));

        if let Err(err) = update_fut.await {
            let _ = command.error(ctx, GENERAL_ISSUE).await;

            return Err(err);
        }

        let content = format!("Successfully set your skin to `{url}`");
        let builder = MessageBuilder::new().embed(content);
        command.update(ctx, &builder).await?;

        Ok(())
    }
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "unset", desc = "Remove the skin that you previously set")]
pub struct UnsetSkin;

impl UnsetSkin {
    async fn process(self, ctx: &Context, command: &InteractionCommand) -> Result<()> {
        let update_fut = ctx.user_config().update_skin(command.user_id()?, None);

        if let Err(err) = update_fut.await {
            let _ = command.error(ctx, GENERAL_ISSUE).await;

            return Err(err);
        }

        let content = "Successfully unset your skin";
        let builder = MessageBuilder::new().embed(content);
        command.update(ctx, &builder).await?;

        Ok(())
    }
}

pub enum ValidationStatus {
    Continue,
    Handled,
}

pub enum SkinValidation {
    Ok,
    Invalid(Reason),
    Err(Report),
}

#[derive(Debug)]
pub enum Reason {
    TooLong,
    InvalidUrl,
    MissingContentDisposition,
    NeitherAttachmentNorInline,
    MissingFilename,
    NotOsk,
}

impl SkinValidation {
    pub async fn check(
        ctx: &Context,
        command: &InteractionCommand,
        skin_url: &str,
    ) -> Result<ValidationStatus> {
        match Self::validate(ctx, skin_url).await {
            SkinValidation::Ok => Ok(ValidationStatus::Continue),
            SkinValidation::Invalid(reason) => {
                debug!(?reason, "Invalid skin url reason");

                let content = "Looks like an invalid skin url.\n\
                    Must be a URL to a direct-download of an .osk file or one of these approved sites:\n\
                    - `https://drive.google.com`\n\
                    - `https://www.dropbox.com`\n\
                    - `https://mega.nz`\n\
                    - `https://www.mediafire.com`\n\
                    - `https://skins.osuck.net`\n\
                    - `https://github.com`\n\
                    If you want to suggest another site let Badewanne3 know";

                command.error(ctx, content).await?;

                Ok(ValidationStatus::Handled)
            }
            SkinValidation::Err(err) => {
                let content = "Failed to validate skin url";
                let _ = command.error(ctx, content).await;

                Err(err.wrap_err("failed to validate skin url"))
            }
        }
    }

    async fn validate(ctx: &Context, skin_url: &str) -> Self {
        if skin_url.len() > 256 {
            return Self::Invalid(Reason::TooLong);
        } else if matcher::is_approved_skin_site(skin_url) {
            return Self::Ok;
        } else if !(skin_url.starts_with("https://") && skin_url.contains('.')) {
            return Self::Invalid(Reason::InvalidUrl);
        }

        let (parts, _) = match ctx.client().check_skin_url(skin_url).await {
            Ok(res) => res.into_parts(),
            Err(err) => return Self::Err(err.into()),
        };

        let Some(content_disposition) = parts.headers.get("Content-Disposition") else {
            return Self::Invalid(Reason::MissingContentDisposition);
        };

        let content_disposition = String::from_utf8_lossy(content_disposition.as_bytes());
        trace!("Content-Disposition: {content_disposition}");

        let mut split = content_disposition.split(';');

        if !matches!(split.next(), Some("attachment" | "inline")) {
            return Self::Invalid(Reason::NeitherAttachmentNorInline);
        };

        let content_opt = split.find_map(|content| {
            content
                .trim_start_matches(' ')
                .trim_start_matches("%20")
                .strip_prefix("filename=")
        });

        let Some(filename) = content_opt else {
            return Self::Invalid(Reason::MissingFilename);
        };

        let filename = filename.trim_matches('"');

        if filename.ends_with(".osk") {
            Self::Ok
        } else {
            Self::Invalid(Reason::NotOsk)
        }
    }
}
