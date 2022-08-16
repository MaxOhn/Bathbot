use std::{borrow::Cow, sync::Arc};

use command_macros::{HasName, SlashCommand};
use handlebars::Handlebars;
use once_cell::sync::Lazy;
use rosu_pp::{osu::OsuScoreState, Beatmap, OsuPP};
use rosu_v2::prelude::{GameMode, GameMods, OsuError, Score};
use serde_json::json;
use twilight_interactions::command::{CommandModel, CreateCommand};
use twilight_model::{
    application::interaction::ApplicationCommand,
    id::{marker::UserMarker, Id},
};

use crate::{
    commands::GameModeOption,
    core::{commands::CommandOrigin, BotConfig, Context},
    error::PpError,
    util::{
        builder::MessageBuilder,
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers::round,
        osu::prepare_beatmap_file,
        ApplicationCommandExt,
    },
    BotResult,
};

use super::{get_user_and_scores, ScoreArgs, UserArgs};

static HTML_TEMPLATE: Lazy<Handlebars<'static>> = Lazy::new(|| {
    let mut handlebars = Handlebars::new();
    let mut path = BotConfig::get().paths.website.to_owned();
    path.push("card.hbs");

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

    let (user, scores) = match get_user_and_scores(&ctx, user_args, &scores_args).await {
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
        "name": user.username,
        "level": user.statistics.map(|stats| round(stats.level.float())).unwrap_or(0.0),
        "acc": round(acc as f32),
        "aim": round(aim as f32),
        "speed": round(speed as f32),
        "title": title,
    });

    let html = match HTML_TEMPLATE.render("card", &render_data) {
        Ok(rendered) => rendered,
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let bytes = match ctx.client().html_to_png(&html).await {
        Ok(bytes) => bytes.to_vec(),
        Err(err) => {
            let _ = orig.error(&ctx, GENERAL_ISSUE).await;

            return Err(err.into());
        }
    };

    let builder = MessageBuilder::new().attachment("card.png", bytes);
    orig.create_message(&ctx, &builder).await?;

    Ok(())
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
enum Skills {
    Osu { acc: f64, aim: f64, speed: f64 },
    Taiko {},
    Catch {},
    Mania {},
}

impl Skills {
    async fn calculate(ctx: &Context, scores: &[Score]) -> Result<Self, PpError> {
        // TODO: Handle modes

        let mut acc = 0.0;
        let mut aim = 0.0;
        let mut speed = 0.0;
        let mut weight_sum = 0.0;

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

            let acc_val = attrs.pp_acc;
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

    fn evaluate_title(self, scores: &[Score]) -> String {
        let mod_adjective = self.process_mods(scores);

        let (acc, aim, speed) = match self {
            Self::Osu { acc, aim, speed } => (acc, aim, speed),
            _ => todo!(),
        };

        let max = acc.max(aim).max(speed);

        const THRESHOLD: f64 = 0.91;

        let skills = [
            acc / max > THRESHOLD,
            aim / max > THRESHOLD,
            speed / max > THRESHOLD,
        ];

        let skill_title = match skills {
            [true, true, true] => "All-Rounder",
            [true, true, false] => "Sniper",
            [true, false, true] => "Ninja",
            [true, false, false] => "Rhythm Enjoyer",
            [false, true, true] => "Gunslinger",
            [false, true, false] => "Whack-A-Mole",
            [false, false, true] => "Masher",
            [false, false, false] => unreachable!(),
        };

        let title_prefix = match max {
            _ if max < 10.0 => "Newbie",
            _ if max < 20.0 => "Novice",
            _ if max < 30.0 => "Rookie",
            _ if max < 40.0 => "Apprentice",
            _ if max < 50.0 => "Advanced",
            _ if max < 60.0 => "Outstanding",
            _ if max < 70.0 => "Seasoned",
            _ if max < 80.0 => "Professional",
            _ if max < 85.0 => "Expert",
            _ if max < 90.0 => "Master",
            _ if max < 95.0 => "Legendary",
            _ => "God",
        };

        format!("{title_prefix} {mod_adjective} {skill_title}")
    }

    fn process_mods(&self, scores: &[Score]) -> Cow<'static, str> {
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
            return "mod-hating".into();
        }

        let mut res = String::new();

        if doubletime > 70 {
            res.push_str("speedy");
        }

        if hardrock > 70 {
            if !res.is_empty() {
                res.push(' ');
            }

            res.push_str("ant-clicking");
        }

        if hidden > 70 {
            if !res.is_empty() {
                res.push(' ');
            }

            res.push_str("HD-abusing");
        }

        if !res.is_empty() {
            res.into()
        } else if nomod < 10 {
            "mod-loving".into()
        } else {
            "versatile".into()
        }
    }
}
