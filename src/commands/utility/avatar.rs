use crate::{
    arguments::{DiscordUserArgs, NameArgs},
    embeds::BasicEmbedData,
    util::{
        discord,
        globals::{AVATAR_URL, OSU_API_ISSUE},
    },
    Osu,
};

use rosu::backend::UserRequest;
use serenity::{
    framework::standard::{macros::command, Args, CommandError, CommandResult},
    model::prelude::Message,
    prelude::Context,
};

#[command]
#[only_in("guild")]
#[description = "Displaying the profile picture of a discord or osu! user.\n\
                For a discord user, just give a mention or a user id.\n\
                For an osu! user, the first argument must be `osu`, \
                the next argument must be their username"]
#[aliases("pfp")]
#[example = "@Badewanne3"]
#[example = "osu Badewanne3"]
#[sub_commands("osu")]
pub async fn avatar(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let user = match DiscordUserArgs::new(args, &ctx, msg.guild_id.unwrap()).await {
        Ok(args) => args.user,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg).await?;
            return Ok(());
        }
    };
    let response = if let Some(url) = user.avatar_url() {
        let user = AvatarUser::Discord {
            name: user.tag(),
            url,
        };
        let data = BasicEmbedData::create_avatar(user);
        msg.channel_id
            .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
            .await?
    } else {
        msg.channel_id
            .say(
                &ctx.http,
                format!("No avatar found for discord user {}", user.name),
            )
            .await?
    };
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

#[command]
pub async fn osu(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let name = match NameArgs::new(args).name {
        Some(name) => name,
        None => {
            msg.channel_id
                .say(&ctx.http, "After `osu` you need to provide a username")
                .await?;
            return Ok(());
        }
    };
    let user = {
        let req = UserRequest::with_username(&name);
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        match req.queue_single(&osu).await {
            Ok(user) => match user {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))
                        .await?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE).await?;
                return Err(CommandError::from(why.to_string()));
            }
        }
    };
    let user = AvatarUser::Osu {
        name: user.username,
        url: format!("{}{}", AVATAR_URL, user.user_id),
    };
    let data = BasicEmbedData::create_avatar(user);
    let response = msg
        .channel_id
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))
        .await?;
    discord::reaction_deletion(&ctx, response, msg.author.id).await;
    Ok(())
}

pub enum AvatarUser {
    Discord { name: String, url: String },
    Osu { name: String, url: String },
}

impl AvatarUser {
    pub fn name(&self) -> &str {
        match self {
            AvatarUser::Discord { name, .. } => &name,
            AvatarUser::Osu { name, .. } => &name,
        }
    }

    pub fn url(&self) -> &str {
        match self {
            AvatarUser::Discord { url, .. } => &url,
            AvatarUser::Osu { url, .. } => &url,
        }
    }
}
