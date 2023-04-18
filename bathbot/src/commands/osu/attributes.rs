use std::sync::Arc;

use bathbot_macros::SlashCommand;
use bathbot_util::{matcher, osu::AttributeKind, MessageBuilder};
use eyre::Result;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CreateCommand};

use crate::{
    core::Context,
    embeds::AttributesEmbed,
    embeds::EmbedData,
    util::{interaction::InteractionCommand, InteractionCommandExt},
};

#[derive(CommandModel, CreateCommand, SlashCommand)]
#[command(name = "attributes")]
#[flags(SKIP_DEFER)]
/// Check how mods influence the AR, OD, HP, or CS attributes
pub enum Attributes {
    #[command(name = "ar")]
    Ar(AttributesAr),
    #[command(name = "cs")]
    Cs(AttributesCs),
    #[command(name = "hp")]
    Hp(AttributesHp),
    #[command(name = "od")]
    Od(AttributesOd),
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "ar")]
/// Check how mods influence the approach rate attribute
pub struct AttributesAr {
    #[command(rename = "value", min_value = -15.0, max_value = 13.0)]
    /// Specify an AR value
    number: f32,
    #[command(
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "cs")]
/// Check how mods influence the circle size attribute
pub struct AttributesCs {
    #[command(rename = "value", min_value = 0.0, max_value = 20.0)]
    /// Specify a CS value
    number: f32,
    #[command(
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "hp")]
/// Check how mods influence the drain rate attribute
pub struct AttributesHp {
    #[command(rename = "value", min_value = 0.0, max_value = 20.0)]
    /// Specify an HP value
    number: f32,
    #[command(
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: String,
}

#[derive(CommandModel, CreateCommand)]
#[command(name = "od")]
/// Check how mods influence the overall difficulty attribute
pub struct AttributesOd {
    #[command(rename = "value", min_value = -13.33, max_value = 13.33)]
    /// Specify an OD value
    number: f32,
    #[command(
        help = "Specify mods either directly or through the explicit `+mod!` / `+mod` syntax, \
        e.g. `hdhr` or `+hdhr!`"
    )]
    /// Specify mods e.g. hdhr or nm
    mods: String,
}

async fn slash_attributes(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let attrs = Attributes::from_interaction(command.input_data())?;

    let (kind, value, mods) = match attrs {
        Attributes::Ar(args) => (AttributeKind::Ar, args.number, args.mods),
        Attributes::Cs(args) => (AttributeKind::Cs, args.number, args.mods),
        Attributes::Hp(args) => (AttributeKind::Hp, args.number, args.mods),
        Attributes::Od(args) => (AttributeKind::Od, args.number, args.mods),
    };

    let mods = if let Ok(mods) = mods.parse() {
        mods
    } else if let Some(mods) = matcher::get_mods(&mods) {
        mods.into_mods()
    } else {
        let content =
            "Failed to parse mods. Be sure to specify a valid mod combination e.g. `hrdt`.";
        command.error_callback(&ctx, content).await?;

        return Ok(());
    };

    let valid_mods = [
        GameMode::Osu,
        GameMode::Taiko,
        GameMode::Catch,
        GameMode::Mania,
    ]
    .into_iter()
    .filter_map(|mode| mods.clone().with_mode(mode))
    .any(|mods| mods.is_valid());

    if !valid_mods {
        let content = "Looks like either some of these mods are incompatible with each other \
            or those mods don't fit to any gamemode.";
        command.error_callback(&ctx, content).await?;

        return Ok(());
    }

    let embed = AttributesEmbed::new(kind, value, mods).build();
    let builder = MessageBuilder::new().embed(embed);
    command.callback(&ctx, builder, false).await?;

    Ok(())
}
