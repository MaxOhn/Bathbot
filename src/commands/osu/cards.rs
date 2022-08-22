use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    sync::Arc,
};

use command_macros::{HasName, SlashCommand};
use handlebars::Handlebars;
use once_cell::sync::Lazy;
use rosu_pp::{osu::OsuScoreState, Beatmap, OsuPP};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use serde::{Serialize, Serializer};
use serde_json::json;
use time::OffsetDateTime;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::GameModeOption,
    core::{commands::CommandOrigin, BotConfig, Context},
    embeds::{CardEmbed, EmbedData},
    error::PpError,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        datetime::DATE_FORMAT,
        osu::{flag_url_svg, prepare_beatmap_file},
        ApplicationCommandExt, HtmlToPng,
    },
    BotResult,
};

use super::{get_user_and_scores, ScoreArgs, UserArgs};

static HTML_TEMPLATE: Lazy<Handlebars<'static>> = Lazy::new(|| {
    let mut handlebars = Handlebars::new();
    let mut path = BotConfig::get().paths.cards.clone();
    path.push("template/template.tmpl");

    handlebars
        .register_template_file("card", path)
        .expect("failed to register card template to handlebars");

    handlebars
});

#[derive(CommandModel, CreateCommand, SlashCommand, HasName)]
#[command(name = "card")]
/// Create a user card
pub struct Card {
    /// Specify a gamemode
    mode: Option<GameModeOption>,
    /// Specify a username
    name: Option<String>,
    #[command(
        help = "Instead of specifying an osu! username with the `name` option, \
        you can use this option to choose a discord user.\n\
        Only works on users who have used the `/link` command."
    )]
    /// Specify a linked discord user
    discord: Option<Id<UserMarker>>,
}

async fn slash_card(ctx: Arc<Context>, mut command: Box<ApplicationCommand>) -> BotResult<()> {
    let args = Card::from_interaction(command.input_data())?;
    let orig = CommandOrigin::Interaction { command };

    let (name, mode) = name_mode!(ctx, orig, args);

    if !matches!(mode, GameMode::Osu) {
        let content =
            "For now cards are only available for osu!standard, other modes will follow soon:tm:";

        return orig.error(&ctx, content).await;
    }

    let user_args = UserArgs::new(&name, mode);
    let scores_args = ScoreArgs::top(100);

    let (mut user, scores) = match get_user_and_scores(&ctx, user_args, &scores_args).await {
        Ok((user, scores)) => (user, scores),
        Err(OsuError::NotFound) => {
            let content = format!("User `{name}` was not found");

            return orig.error(&ctx, content).await;
        }
        Err(err) => {
            let _ = orig.error(&ctx, OSU_API_ISSUE).await;

            return Err(err.into());
        }
    };

    user.mode = mode;
    let stats = user.statistics.as_ref().expect("missing user stats");

    let skills = match Skills::calculate(&ctx, &scores).await {
        Ok(skills) => skills,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let title = skills.evaluate_title(&scores);

    let (acc, aim, speed) = match skills {
        Skills::Osu { acc, aim, speed } => (acc, aim, speed),
        _ => todo!(),
    };

    let render_data = json!({
        "path": BotConfig::get().paths.cards,
        "gamemode": match user.mode {
            GameMode::Osu => "mode_standard",
            GameMode::Taiko => "mode_taiko",
            GameMode::Catch => "mode_catch",
            GameMode::Mania => "mode_mania",
        },
        "title": title,
        "username": user.username,
        "flag": flag_url_svg(&user.country_code),
        "gamemode_icon": match user.mode {
            GameMode::Osu => "img/gamemodes/Standard.svg",
            GameMode::Taiko => "img/gamemodes/Taiko.svg",
            GameMode::Catch => "img/gamemodes/Catch.svg",
            GameMode::Mania => "img/gamemodes/Mania.svg",
        },
        "user_pfp": user.avatar_url,
        "accuracy_enabled": "show",
        "accuracy": acc.trunc(),
        "accuracy_decimal": (acc.fract() * 100.0).round() as u32,
        "aim_enabled": "show",
        "aim": aim.trunc(),
        "aim_decimal": (aim.fract() * 100.0).round() as u32,
        "speed_enabled": "show",
        "speed": speed.trunc(),
        "speed_decimal": (speed.fract() * 100.0).round() as u32,
        "global_rank": stats.global_rank.unwrap_or(0),
        "country_rank": stats.country_rank.unwrap_or(0),
        "level": stats.level.current,
        "level_percentage": stats.level.progress,
        "date": OffsetDateTime::now_utc().format(&DATE_FORMAT).unwrap(),
        "background_image": format!("img/backgrounds/{}.png", title.prefix.background()),
    });

    let html = match HTML_TEMPLATE.render("card", &render_data) {
        Ok(rendered) => rendered,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let bytes = match HtmlToPng::convert(&html) {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let embed = CardEmbed::new(&user).build();

    let builder = MessageBuilder::new()
        .attachment("card.png", bytes)
        .embed(embed);

    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[derive(Copy, Clone)]
enum Skills {
    Osu {
        acc: f64,
        aim: f64,
        speed: f64,
    },
    #[allow(dead_code)]
    Taiko {},
    #[allow(dead_code)]
    Catch {},
    #[allow(dead_code)]
    Mania {},
}

impl Skills {
    async fn calculate(ctx: &Context, scores: &[Score]) -> Result<Self, PpError> {
        // TODO: Handle modes

        let mut acc = 0.0;
        let mut aim = 0.0;
        let mut speed = 0.0;
        let mut weight_sum = 0.0;

        const ACC_NERF: f64 = 1.3;
        const AIM_NERF: f64 = 2.6;
        const SPEED_NERF: f64 = 2.4;

        for (i, score) in scores.iter().enumerate() {
            let map = score.map.as_ref().unwrap();
            let map_path = prepare_beatmap_file(ctx, map.map_id).await?;
            let map = Beatmap::from_path(map_path).await?;

            let state = OsuScoreState {
                max_combo: score.max_combo as usize,
                n300: score.statistics.count_300 as usize,
                n100: score.statistics.count_100 as usize,
                n50: score.statistics.count_50 as usize,
                misses: score.statistics.count_miss as usize,
            };

            let attrs = OsuPP::new(&map)
                .mods(score.mods.bits())
                .state(state)
                .calculate();

            let acc_val = attrs.pp_acc / ACC_NERF;
            let aim_val = attrs.pp_aim / AIM_NERF;
            let speed_val = attrs.pp_speed / SPEED_NERF;
            let weight = 0.95_f64.powi(i as i32);

            acc += acc_val * weight;
            aim += aim_val * weight;
            speed += speed_val * weight;
            weight_sum += weight;
        }

        // https://www.desmos.com/calculator/gqnhbpa0d3
        let map = |val: f64| {
            let factor = (8.0 / (val / 72.0 + 8.0)).powi(10);

            -101.0 * factor + 101.0
        };

        acc = map(acc / weight_sum);
        aim = map(aim / weight_sum);
        speed = map(speed / weight_sum);

        Ok(Self::Osu { acc, aim, speed })
    }

    fn evaluate_title(self, scores: &[Score]) -> Title {
        let (acc, aim, speed) = match self {
            Self::Osu { acc, aim, speed } => (acc, aim, speed),
            _ => todo!(),
        };

        let max = acc.max(aim).max(speed);
        let prefix = TitlePrefix::new(max);
        let mods = ModDescriptions::new(scores);
        let main = TitleMain::new(acc, aim, speed, max);

        Title { prefix, mods, main }
    }
}

struct Title {
    prefix: TitlePrefix,
    mods: ModDescriptions,
    main: TitleMain,
}

impl Display for Title {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{} {} {}", self.prefix, self.mods, self.main)
    }
}

impl Serialize for Title {
    #[inline]
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug)]
#[repr(u8)]
enum TitlePrefix {
    Newbie,
    Novice,
    Rookie,
    Apprentice,
    Advanced,
    Outstanding,
    Seasoned,
    Professional,
    Expert,
    Master,
    Legendary,
    God,
}

impl TitlePrefix {
    fn new(value: f64) -> Self {
        match value {
            _ if value < 10.0 => Self::Newbie,
            _ if value < 20.0 => Self::Novice,
            _ if value < 30.0 => Self::Rookie,
            _ if value < 40.0 => Self::Apprentice,
            _ if value < 50.0 => Self::Advanced,
            _ if value < 60.0 => Self::Outstanding,
            _ if value < 70.0 => Self::Seasoned,
            _ if value < 80.0 => Self::Professional,
            _ if value < 85.0 => Self::Expert,
            _ if value < 90.0 => Self::Master,
            _ if value < 95.0 => Self::Legendary,
            _ => Self::God,
        }
    }

    fn background(&self) -> &'static str {
        match self {
            TitlePrefix::Newbie => "newbie",
            TitlePrefix::Novice => "novice",
            TitlePrefix::Rookie => "rookie",
            TitlePrefix::Apprentice => "apprentice",
            TitlePrefix::Advanced => "advanced",
            TitlePrefix::Outstanding => "outstanding",
            TitlePrefix::Seasoned => "seasoned",
            TitlePrefix::Professional => "professional",
            TitlePrefix::Expert => "expert",
            TitlePrefix::Master => "master",
            TitlePrefix::Legendary => "legendary",
            TitlePrefix::God => "god",
        }
    }
}

impl Display for TitlePrefix {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Debug>::fmt(self, f)
    }
}

enum ModDescription {
    ModHating,
    Speedy,
    AntClicking,
    HdAbusing,
    ModLoving,
    Versatile,
}

impl Display for ModDescription {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let desc = match self {
            ModDescription::ModHating => "mod-hating",
            ModDescription::Speedy => "speedy",
            ModDescription::AntClicking => "ant-clicking",
            ModDescription::HdAbusing => "HD-abusing",
            ModDescription::ModLoving => "mod-loving",
            ModDescription::Versatile => "versatile",
        };

        f.write_str(desc)
    }
}

struct ModDescriptions(Vec<ModDescription>);

impl ModDescriptions {
    fn new(scores: &[Score]) -> Self {
        let mut nomod = 0;
        let mut hidden = 0;
        let mut doubletime = 0;
        let mut hardrock = 0;

        for score in scores {
            if score.mods.is_empty() {
                nomod += 1;
                continue;
            }

            hidden += score.mods.contains(GameMods::Hidden) as usize;
            doubletime += score.mods.contains(GameMods::DoubleTime) as usize;
            hardrock += score.mods.contains(GameMods::HardRock) as usize;
        }

        if nomod > 70 {
            return ModDescription::ModHating.into();
        }

        let mut mods = Self(Vec::new());

        if doubletime > 70 {
            mods.push(ModDescription::Speedy);
        }

        if hardrock > 70 {
            mods.push(ModDescription::AntClicking);
        }

        if hidden > 70 {
            mods.push(ModDescription::HdAbusing);
        }

        if !mods.is_empty() {
            mods
        } else if nomod < 10 {
            ModDescription::ModLoving.into()
        } else {
            ModDescription::Versatile.into()
        }
    }

    fn push(&mut self, desc: ModDescription) {
        self.0.push(desc);
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<ModDescription> for ModDescriptions {
    #[inline]
    fn from(desc: ModDescription) -> Self {
        Self(vec![desc])
    }
}

impl Display for ModDescriptions {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut iter = self.0.iter();

        if let Some(desc) = iter.next() {
            write!(f, "{desc}")?;

            for desc in iter {
                write!(f, " {desc}")?;
            }
        }

        Ok(())
    }
}

enum TitleMain {
    AllRounder,
    Sniper,
    Ninja,
    RhythmEnjoyer,
    Gunslinger,
    WhackAMole,
    Masher,
}

impl TitleMain {
    fn new(acc: f64, aim: f64, speed: f64, max: f64) -> Self {
        const THRESHOLD: f64 = 0.91;
        let map = |val| val / max > THRESHOLD;

        match (map(acc), map(aim), map(speed)) {
            (true, true, true) => Self::AllRounder,
            (true, true, false) => Self::Sniper,
            (true, false, true) => Self::Ninja,
            (true, false, false) => Self::RhythmEnjoyer,
            (false, true, true) => Self::Gunslinger,
            (false, true, false) => Self::WhackAMole,
            (false, false, true) => Self::Masher,
            (false, false, false) => unreachable!(),
        }
    }
}

impl Display for TitleMain {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let main = match self {
            TitleMain::AllRounder => "All-Rounder",
            TitleMain::Sniper => "Sniper",
            TitleMain::Ninja => "Ninja",
            TitleMain::RhythmEnjoyer => "Rhythm Enjoyer",
            TitleMain::Gunslinger => "Gunslinger",
            TitleMain::WhackAMole => "Whack-A-Mole",
            TitleMain::Masher => "Masher",
        };

        f.write_str(main)
    }
}
