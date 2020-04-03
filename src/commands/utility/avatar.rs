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
use tokio::runtime::Runtime;

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
fn avatar(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let user = match DiscordUserArgs::new(args, &ctx, msg.guild_id.unwrap()) {
        Ok(args) => args.user,
        Err(err_msg) => {
            msg.channel_id.say(&ctx.http, err_msg)?;
            return Ok(());
        }
    };
    let response = if let Some(url) = user.avatar_url() {
        let user = AvatarUser::Discord {
            name: user.tag(),
            url: url,
        };
        let data = BasicEmbedData::create_avatar(user);
        msg.channel_id
            .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?
    } else {
        msg.channel_id.say(
            &ctx.http,
            format!("No avatar found for discord user {}", user.name),
        )?
    };
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
    Ok(())
}

#[command]
fn osu(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    let name = match NameArgs::new(args).name {
        Some(name) => name,
        None => {
            msg.channel_id
                .say(&ctx.http, "After `osu` you need to provide a username")?;
            return Ok(());
        }
    };
    let user = {
        let req = UserRequest::with_username(&name);
        let mut rt = Runtime::new().unwrap();
        let data = ctx.data.read();
        let osu = data.get::<Osu>().expect("Could not get Osu");
        match rt.block_on(req.queue_single(&osu)) {
            Ok(user) => match user {
                Some(user) => user,
                None => {
                    msg.channel_id
                        .say(&ctx.http, format!("User `{}` was not found", name))?;
                    return Ok(());
                }
            },
            Err(why) => {
                msg.channel_id.say(&ctx.http, OSU_API_ISSUE)?;
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
        .send_message(&ctx.http, |m| m.embed(|e| data.build(e)))?;
    discord::save_response_owner(response.id, msg.author.id, ctx.data.clone());
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
