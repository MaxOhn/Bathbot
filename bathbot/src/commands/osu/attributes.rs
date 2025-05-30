use std::borrow::Cow;

use bathbot_macros::{SlashCommand, command};
use bathbot_util::{
    MessageBuilder, matcher,
    osu::{AttributeKind, ModSelection},
};
use eyre::Result;
use rosu_v2::{model::mods::GameModsIntermode, prelude::GameMode};
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::commands::{CommandOrigin, prefix::Args},
    embeds::{AttributesEmbed, EmbedData},
    util::{ChannelExt, InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(
    name = "attributes",
    desc = "Check how mods influence the AR, OD, HP, or CS attributes"
)]
#[flags(SKIP_DEFER)]
pub enum Attributes<'a> {
    #[command(name = "ar")]
    Ar(AttributesAr<'a>),
    #[command(name = "cs")]
    Cs(AttributesCs<'a>),
    #[command(name = "hp")]
    Hp(AttributesHp<'a>),
    #[command(name = "od")]
    Od(AttributesOd<'a>),
}

impl<'a> Attributes<'a> {
    fn args(kind: AttributeKind, mut args: Args<'a>) -> Result<Self, &'static str> {
        let number: f32 = args
            .next()
            .map(str::parse)
            .and_then(Result::ok)
            .ok_or("The first argument must be a number")?;

        let mods = args
            .next()
            .map(Cow::Borrowed)
            .ok_or("The second argument must be mods")?;

        let this = match kind {
            AttributeKind::Ar => Self::Ar(AttributesAr {
                number: number.clamp(AR_MIN, AR_MAX),
                mods,
                clock_rate: None,
            }),
            AttributeKind::Cs => Self::Cs(AttributesCs {
                number: number.clamp(CS_MIN, CS_MAX),
                mods,
                clock_rate: None,
            }),
            AttributeKind::Hp => Self::Hp(AttributesHp {
                number: number.clamp(HP_MIN, HP_MAX),
                mods,
                clock_rate: None,
            }),
            AttributeKind::Od => Self::Od(AttributesOd {
                number: number.clamp(OD_MIN, OD_MAX),
                mods,
                clock_rate: None,
            }),
        };

        Ok(this)
    }
}

const AR_DESC: &str = "Check how mods influence the approach rate attribute";
const AR_MIN: f32 = -15.0;
const AR_MAX: f32 = 13.0;

#[derive(CommandModel, CreateCommand)]
#[command(name = "ar", desc = AR_DESC)]
pub struct AttributesAr<'a> {
    #[command(
        rename = "value",
        min_value = AR_MIN = f32,
        max_value = AR_MAX = f32,
        desc = "Specify an AR value"
    )]
    number: f32,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a custom clock rate that overwrites mods")]
    clock_rate: Option<f32>,
}

const CS_DESC: &str = "Check how mods influence the circle size attribute";
const CS_MIN: f32 = 0.0;
const CS_MAX: f32 = 20.0;

#[derive(CommandModel, CreateCommand)]
#[command(name = "cs", desc = CS_DESC)]
pub struct AttributesCs<'a> {
    #[command(
        rename = "value",
        min_value = CS_MIN = f32,
        max_value = CS_MAX = f32,
        desc = "Specify a CS value"
    )]
    number: f32,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a custom clock rate that overwrites mods")]
    clock_rate: Option<f32>,
}

const HP_DESC: &str = "Check how mods influence the drain rate attribute";
const HP_MIN: f32 = 0.0;
const HP_MAX: f32 = 20.0;

#[derive(CommandModel, CreateCommand)]
#[command(name = "hp", desc = HP_DESC)]
pub struct AttributesHp<'a> {
    #[command(
        rename = "value",
        min_value = HP_MIN = f32,
        max_value = HP_MAX = f32,
        desc = "Specify an HP value"
    )]
    number: f32,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a custom clock rate that overwrites mods")]
    clock_rate: Option<f32>,
}

const OD_DESC: &str = "Check how mods influence the overall difficulty attribute";
const OD_MIN: f32 = -13.33;
const OD_MAX: f32 = 13.33;

#[derive(CommandModel, CreateCommand)]
#[command(name = "od", desc = OD_DESC)]
pub struct AttributesOd<'a> {
    #[command(
        rename = "value",
        min_value = OD_MIN = f32,
        max_value = OD_MAX = f32,
        desc = "Specify an OD value"
    )]
    number: f32,
    #[command(
        desc = "Specify mods e.g. hdhr or nm",
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    mods: Cow<'a, str>,
    #[command(desc = "Specify a custom clock rate that overwrites mods")]
    clock_rate: Option<f32>,
}

async fn slash_attributes(mut command: InteractionCommand) -> Result<()> {
    let attrs = Attributes::from_interaction(command.input_data())?;

    attributes((&mut command).into(), attrs).await
}

#[command]
#[desc(AR_DESC)]
#[usage("[number] [mods]")]
#[examples("8.5 +dt")]
#[aliases("approachrate")]
#[group(AllModes)]
async fn prefix_ar(msg: &Message, args: Args<'_>) -> Result<()> {
    match Attributes::args(AttributeKind::Ar, args) {
        Ok(args) => attributes(msg.into(), args).await,
        Err(err) => {
            msg.error(err).await?;

            Ok(())
        }
    }
}

#[command]
#[desc(CS_DESC)]
#[usage("[number] [mods]")]
#[examples("4 +hr")]
#[aliases("circlesize")]
#[group(AllModes)]
async fn prefix_cs(msg: &Message, args: Args<'_>) -> Result<()> {
    match Attributes::args(AttributeKind::Cs, args) {
        Ok(args) => attributes(msg.into(), args).await,
        Err(err) => {
            msg.error(err).await?;

            Ok(())
        }
    }
}

#[command]
#[desc(HP_DESC)]
#[usage("[number] [mods]")]
#[examples("2 +dthr")]
#[aliases("dr", "drainrate")]
#[group(AllModes)]
async fn prefix_hp(msg: &Message, args: Args<'_>) -> Result<()> {
    match Attributes::args(AttributeKind::Hp, args) {
        Ok(args) => attributes(msg.into(), args).await,
        Err(err) => {
            msg.error(err).await?;

            Ok(())
        }
    }
}

#[command]
#[desc(OD_DESC)]
#[usage("[number] [mods]")]
#[examples("5 +hddt")]
#[aliases("overalldifficulty")]
#[group(AllModes)]
async fn prefix_od(msg: &Message, args: Args<'_>) -> Result<()> {
    match Attributes::args(AttributeKind::Od, args) {
        Ok(args) => attributes(msg.into(), args).await,
        Err(err) => {
            msg.error(err).await?;

            Ok(())
        }
    }
}

async fn attributes(orig: CommandOrigin<'_>, args: Attributes<'_>) -> Result<()> {
    let (kind, value, mods, clock_rate) = match args {
        Attributes::Ar(args) => (AttributeKind::Ar, args.number, args.mods, args.clock_rate),
        Attributes::Cs(args) => (AttributeKind::Cs, args.number, args.mods, args.clock_rate),
        Attributes::Hp(args) => (AttributeKind::Hp, args.number, args.mods, args.clock_rate),
        Attributes::Od(args) => (AttributeKind::Od, args.number, args.mods, args.clock_rate),
    };

    let mods = if let Some(mods) = GameModsIntermode::try_from_acronyms(&mods) {
        mods
    } else {
        match matcher::get_mods(&mods) {
            Some(ModSelection::Include(mods) | ModSelection::Exact(mods)) => mods,
            None => {
                let content =
                    "Failed to parse mods. Be sure to specify a valid mod combination e.g. `hrdt`.";
                orig.error_callback(content).await?;

                return Ok(());
            }
            Some(ModSelection::Exclude { .. }) => {
                let content = "Excluding mods does not work for this command";
                orig.error_callback(content).await?;

                return Ok(());
            }
        }
    };

    let valid_mods = [
        GameMode::Osu,
        GameMode::Taiko,
        GameMode::Catch,
        GameMode::Mania,
    ]
    .into_iter()
    .any(|mode| mods.clone().with_mode(mode).is_valid());

    if !valid_mods {
        let content = "Looks like either some of these mods are incompatible with each other \
            or those mods don't fit to any gamemode.";
        orig.error_callback(content).await?;

        return Ok(());
    }

    let embed = AttributesEmbed::new(kind, value, mods, clock_rate).build();
    let builder = MessageBuilder::new().embed(embed);
    orig.callback(builder).await?;

    Ok(())
}
