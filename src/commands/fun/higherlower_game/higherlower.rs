use std::fmt::Write;

use command_macros::SlashCommand;
use dashmap::mapref::entry::Entry;
use eyre::Report;
use image::{png::PngEncoder, ColorType, GenericImageView, ImageBuffer};
use rand::Rng;
use rosu_v2::prelude::{CountryCode, GameMode, GameMods, Grade, Username};
use twilight_interactions::command::CreateCommand;
use twilight_model::{
    application::{
        component::{button::ButtonStyle, ActionRow, Button, Component},
        interaction::{ApplicationCommand, MessageComponentInteraction},
    },
    channel::embed::{Embed, EmbedField},
    id::{
        marker::{MessageMarker, UserMarker},
        Id,
    },
};

use crate::{
    embeds::get_mods,
    util::{
        builder::{EmbedBuilder, MessageBuilder},
        constants::{HL_IMAGE_CHANNEL_ID, RED},
        numbers::{round, with_comma_int},
        osu::grade_emote,
        ApplicationCommandExt, Authored, ChannelExt, ComponentExt,
    },
    BotResult, Context,
};

use std::{borrow::Cow, mem, sync::Arc};

const W: u32 = 900;
const H: u32 = 250;
const ALPHA_THRESHOLD: u8 = 20;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "higherlower",
    help = "Play a game of osu! themed higher lower.\n\
    The available modes are:\n \
    - `PP`: Guess whether the next play is worth higher or lower PP!"
)]
/// Play a game of osu! themed higher lower
pub struct HigherLower;

#[derive(CreateCommand, SlashCommand)]
#[command(
    name = "higherlower",
    help = "Play a game of osu! themed higher lower.\n\
    The available modes are:\n \
    - `PP`: Guess whether the next play is worth higher or lower PP!"
)]
/// Play a game of osu! themed higher lower
pub struct Hl;

async fn slash_higherlower(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    let user = command.user_id()?;

    if ctx.hl_games().contains_key(&user) {
        let content =
            "You can't play two higher lower games at once! Finish your other game first.";

        command.error(&ctx, content).await?;
    } else {
        let play1 = random_play(&ctx).await?;
        let mut play2 = random_play(&ctx).await?;
        while play2 == play1 {
            play2 = random_play(&ctx).await?;
        }

        let mut game = HlGameState {
            previous: play1,
            next: play2,
            player: user,
            id: Id::new(1),
            current_score: 0,
        };

        let image = game.create_image(&ctx).await?;
        let components = hl_components();
        let embed = game.to_embed(image);

        let builder = MessageBuilder::new().embed(embed).components(components);
        let response = command.update(&ctx, &builder).await?.model().await?;

        game.id = response.id;
        ctx.hl_games().insert(user, game);
    }

    Ok(())
}

async fn slash_hl(ctx: Arc<Context>, command: Box<ApplicationCommand>) -> BotResult<()> {
    slash_higherlower(ctx, command).await
}

async fn random_play(ctx: &Context) -> BotResult<HlGameStateInfo> {
    let (page, idx, play): (u32, u32, u32) = {
        let mut rng = rand::thread_rng();

        (
            rng.gen_range(1..=200),
            rng.gen_range(0..50),
            rng.gen_range(0..100),
        )
    };

    // ! Currently 3 requests, can probably be reduced
    let player = ctx
        .osu()
        .performance_rankings(GameMode::STD)
        .page(page)
        .await?
        .ranking
        .swap_remove(idx as usize);

    let play = ctx
        .osu()
        .user_scores(player.user_id)
        .limit(1)
        .offset(play as usize)
        .mode(GameMode::STD)
        .best()
        .await?
        .swap_remove(0);

    let map_id = play.map.as_ref().unwrap().map_id;

    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(err) => {
                return Err(err.into());
            }
        },
    };

    let rank = player
        .statistics
        .as_ref()
        .and_then(|stats| stats.global_rank)
        .unwrap_or(0);

    let mapset = map.mapset.unwrap();

    Ok(HlGameStateInfo {
        user_id: player.user_id,
        username: player.username,
        avatar: player.avatar_url,
        rank,
        country_code: player.country_code,
        map_id: map.map_id,
        map_string: format!(
            "[{} - {} [{}]]({})",
            mapset.artist, mapset.title, map.version, map.url
        ),
        mods: play.mods,
        pp: round(play.pp.unwrap_or(0.0)),
        combo: play.max_combo,
        max_combo: map.max_combo.unwrap_or(0),
        score: play.score,
        acc: round(play.accuracy),
        grade: play.grade,
        cover: mapset.covers.cover,
    })
}

pub struct HlGameState {
    previous: HlGameStateInfo,
    next: HlGameStateInfo,
    #[allow(unused)] // TODO
    player: Id<UserMarker>,
    id: Id<MessageMarker>,
    current_score: u32,
}

impl HlGameState {
    fn to_embed(&self, image: String) -> Embed {
        let title = "Higher or Lower: PP";
        let mut fields = Vec::new();
        fields.push(EmbedField {
            inline: false,
            name: format!("__Previous:__ {}", self.previous.player_string()),
            value: self.previous.play_string(true),
        });
        fields.push(EmbedField {
            inline: false,
            name: format!("__Next:__ {}", self.next.player_string()),
            value: self.next.play_string(false),
        });
        let footer = format!("Current score: {}", self.current_score);
        let embed = EmbedBuilder::new()
            .title(title)
            .fields(fields)
            .image(image)
            .footer(footer)
            .build();

        info!("{:#?}", embed);

        embed
    }

    fn check_guess(&self, guess: HlGuess) -> bool {
        match guess {
            HlGuess::Higher => self.next.pp >= self.previous.pp,
            HlGuess::Lower => self.next.pp <= self.previous.pp,
        }
    }

    async fn create_image(&self, ctx: &Context) -> BotResult<String> {
        let pfp_left =
            image::load_from_memory(&ctx.client().get_avatar(&self.previous.avatar).await?)?
                .thumbnail(128, 128);
        let pfp_right =
            image::load_from_memory(&ctx.client().get_avatar(&self.next.avatar).await?)?
                .thumbnail(128, 128);

        let bg_left =
            image::load_from_memory(&ctx.client().get_mapset_cover(&self.previous.cover).await?)?;

        let bg_right =
            image::load_from_memory(&ctx.client().get_mapset_cover(&self.next.cover).await?)?;

        let mut blipped = ImageBuffer::new(W, H);

        let iter = blipped
            .enumerate_pixels_mut()
            .zip(bg_left.pixels())
            .zip(bg_right.pixels());

        for (((x, _, pixel), (.., left)), (.., right)) in iter {
            *pixel = if x <= W / 2 { left } else { right };
        }

        for (x, y, pixel) in pfp_left.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(x, y, pixel);
            }
        }

        let pfp_right_width = pfp_right.width();

        for (x, y, pixel) in pfp_right.pixels() {
            if pixel.0[3] > ALPHA_THRESHOLD {
                blipped.put_pixel(W - pfp_right_width + x, y, pixel);
            }
        }

        let mut png_bytes: Vec<u8> = Vec::with_capacity((W * H * 4) as usize);
        let png_encoder = PngEncoder::new(&mut png_bytes);
        png_encoder.encode(&blipped.as_raw(), W, H, ColorType::Rgba8)?;

        let builder = MessageBuilder::new().attachment("higherlower.png", png_bytes);

        let mut message = HL_IMAGE_CHANNEL_ID
            .create_message(ctx, &builder)
            .await?
            .model()
            .await?;

        info!("{:?}", message.attachments.first().unwrap().proxy_url);
        return Ok(message.attachments.pop().unwrap().url);
    }
}

struct HlGameStateInfo {
    user_id: u32,
    username: Username,
    avatar: String,
    rank: u32,
    country_code: CountryCode,
    map_id: u32,
    map_string: String,
    mods: GameMods,
    pp: f32,
    combo: u32,
    max_combo: u32,
    score: u32,
    acc: f32,
    grade: Grade,
    cover: String,
}

impl PartialEq for HlGameStateInfo {
    fn eq(&self, other: &Self) -> bool {
        self.user_id == other.user_id && self.map_id == other.map_id
    }
}

impl HlGameStateInfo {
    fn player_string(&self) -> String {
        format!(
            ":flag_{}: {} (#{})",
            self.country_code.to_lowercase(),
            self.username,
            self.rank,
        )
    }

    fn play_string(&self, pp_visible: bool) -> String {
        format!(
            "**{} {}**\n{} {} • **{}%** • **{}x**/{}x • **{}pp**",
            self.map_string,
            get_mods(self.mods),
            grade_emote(self.grade),
            with_comma_int(self.score),
            self.acc,
            self.combo,
            self.max_combo,
            if pp_visible {
                self.pp.to_string().into()
            } else {
                Cow::Borrowed("???")
            }
        )
    }
}

fn hl_components() -> Vec<Component> {
    let higher_button = Button {
        custom_id: Some("higher_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Higher".to_owned()),
        style: ButtonStyle::Success,
        url: None,
    };

    let lower_button = Button {
        custom_id: Some("lower_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Lower".to_owned()),
        style: ButtonStyle::Danger,
        url: None,
    };

    let button_row = ActionRow {
        components: vec![
            Component::Button(higher_button),
            Component::Button(lower_button),
        ],
    };

    vec![Component::ActionRow(button_row)]
}

pub async fn handle_higher(
    ctx: Arc<Context>,
    mut component: MessageComponentInteraction,
) -> BotResult<()> {
    let user = component.user_id()?;

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        let game = entry.get_mut();
        defer_update(&ctx, &mut component, game).await?;

        if game.id != component.message.id {
            return Ok(());
        }

        if !game.check_guess(HlGuess::Higher) {
            game_over(&ctx, &component, game).await?;
            entry.remove();
        } else {
            correct_guess(&ctx, &component, game).await?;
        }
    }

    Ok(())
}

pub async fn handle_lower(
    ctx: Arc<Context>,
    mut component: MessageComponentInteraction,
) -> BotResult<()> {
    let user = component.user_id()?;

    if let Entry::Occupied(mut entry) = ctx.hl_games().entry(user) {
        let game = entry.get_mut();
        defer_update(&ctx, &mut component, game).await?;

        if game.id != component.message.id {
            return Ok(());
        }

        if !game.check_guess(HlGuess::Lower) {
            game_over(&ctx, &component, game).await?;
            entry.remove();
        } else {
            correct_guess(&ctx, &component, game).await?;
        }
    }

    Ok(())
}

async fn correct_guess(
    ctx: &Context,
    component: &MessageComponentInteraction,
    game: &mut HlGameState,
) -> BotResult<()> {
    std::mem::swap(&mut game.previous, &mut game.next);
    game.next = random_play(&ctx).await?;

    while game.next == game.previous {
        game.next = random_play(&ctx).await?;
    }

    game.current_score += 1;
    let image = game.create_image(&ctx).await?;

    let embed = game.to_embed(image);
    let builder = MessageBuilder::new().embed(embed);
    component.update(&ctx, &builder).await?;

    Ok(())
}

async fn game_over(
    ctx: &Context,
    component: &MessageComponentInteraction,
    game: &HlGameState,
) -> BotResult<()> {
    let title = "Game over!";

    let content = format!(
        "You achieved a total score of {}! This is your new personal best!",
        game.current_score
    );

    let embed = EmbedBuilder::new()
        .title(title)
        .description(content)
        .color(RED)
        .build();

    let builder = MessageBuilder::new().embed(embed).components(Vec::new());
    component.update(&ctx, &builder).await?;

    Ok(())
}

async fn defer_update(
    ctx: &Context,
    component: &mut MessageComponentInteraction,
    game: &HlGameState,
) -> BotResult<()> {
    let mut embeds = mem::take(&mut component.message.embeds);
    if let Some(embed) = embeds.first_mut() {
        if let Some(field) = embed.fields.get_mut(1) {
            field.value.truncate(field.value.len() - 7);
            let _ = write!(field.value, "{}pp**", round(game.next.pp));
        }
        if let Some(footer) = embed.footer.as_mut() {
            footer.text += " • Comparing Results...";
        }
    }

    // let response_data = InteractionResponseData {
    //     allowed_mentions: None,
    //     components: None,
    //     content: None,
    //     embeds: Some(embeds),
    //     flags: None,
    //     tts: None,
    //     attachments: None,
    //     choices: None,
    //     custom_id: None,
    //     title: None,
    // };

    // let response = InteractionResponse {
    //     kind: InteractionResponseType::UpdateMessage,
    //     data: Some(response_data),
    // };
    let client = ctx.interaction();

    client
        .update_response(&component.token)
        .embeds(Some(&embeds))?
        .exec()
        .await?;

    // client
    //     .interaction_callback(component.id, &component.token, &response)
    //     .exec()
    //     .await?;

    Ok(())
}

enum HlGuess {
    Higher,
    Lower,
}
