use crate::{
    arguments::{Args, NameFloatArgs},
    custom_client::RankParam,
    embeds::{EmbedData, WhatIfEmbed},
    tracking::process_tracking,
    unwind_error,
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{collections::HashMap, sync::Arc};
use twilight_model::channel::Message;

async fn whatif_main(
    mode: GameMode,
    ctx: Arc<Context>,
    msg: &Message,
    args: Args<'_>,
) -> BotResult<()> {
    let args = match NameFloatArgs::new(&ctx, args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };
    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };
    let pp = args.float;
    if pp < 0.0 {
        let content = "The pp number must be non-negative";
        return msg.error(&ctx, content).await;
    } else if pp > (i64::MAX / 1024) as f32 {
        let content = "Number too large";
        return msg.error(&ctx, content).await;
    }

    // Retrieve the user and their top scores
    let user_fut = ctx.osu().user(name.as_str()).mode(mode);
    let scores_fut = ctx.osu().top_scores(name.as_str()).mode(mode).limit(100);
    let join_result = tokio::try_join!(user_fut, scores_fut);
    let (user, scores) = match join_result {
        Ok((Some(user), scores)) => (user, scores),
        Ok((None, _)) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };

    // Process user and their top scores for tracking
    let mut maps = HashMap::new();
    process_tracking(&ctx, mode, &scores, Some(&user), &mut maps).await;

    let data = if scores.is_empty() {
        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");
                None
            }
        };

        WhatIfData::NoScores { rank }
    } else if pp < scores.last().unwrap().pp.unwrap_or(0.0) {
        WhatIfData::NonTop100
    } else {
        let pp_values: Vec<f32> = scores.iter().filter_map(|score| score.pp).collect();

        let mut actual: f32 = 0.0;
        let mut factor: f32 = 1.0;

        for score in pp_values.iter() {
            actual += score * factor;
            factor *= 0.95;
        }

        let bonus_pp = user.pp_raw - actual;
        let mut potential = 0.0;
        let mut used = false;
        let mut new_pos = scores.len();
        let mut factor = 1.0;

        for (i, &pp_value) in pp_values.iter().enumerate().take(pp_values.len() - 1) {
            if !used && pp_value < pp {
                used = true;
                potential += pp * factor;
                factor *= 0.95;
                new_pos = i + 1;
            }
            potential += pp_value * factor;
            factor *= 0.95;
        }

        if !used {
            potential += pp * factor;
        };

        let new_pp = potential;
        let max_pp = pp_values.get(0).copied().unwrap_or(0.0);

        let rank_result = ctx
            .clients
            .custom
            .get_rank_data(mode, RankParam::Pp(new_pp + bonus_pp))
            .await;

        let rank = match rank_result {
            Ok(rank_pp) => Some(rank_pp.rank),
            Err(why) => {
                unwind_error!(warn, why, "Error while getting rank pp: {}");
                None
            }
        };

        WhatIfData::Top100 {
            bonus_pp,
            new_pp,
            new_pos,
            max_pp,
            rank,
        }
    };

    // Sending the embed
    let embed = WhatIfEmbed::new(user, pp, data).build_owned().build()?;
    msg.build_response(&ctx, |m| m.embed(embed)).await?;
    Ok(())
}

#[command]
#[short_desc("Display the impact of a new X pp score for a user")]
#[long_desc(
    "Calculate the gain in pp if the user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wi")]
pub async fn whatif(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    whatif_main(GameMode::STD, ctx, msg, args).await
}

#[command]
#[short_desc("Display the impact of a new X pp score for a mania user")]
#[long_desc(
    "Calculate the gain in pp if the mania user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wim")]
pub async fn whatifmania(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    whatif_main(GameMode::MNA, ctx, msg, args).await
}

#[command]
#[short_desc("Display the impact of a new X pp score for a taiko user")]
#[long_desc(
    "Calculate the gain in pp if the taiko user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wit")]
pub async fn whatiftaiko(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    whatif_main(GameMode::TKO, ctx, msg, args).await
}

#[command]
#[short_desc("Display the impact of a new X pp score for a ctb user")]
#[long_desc(
    "Calculate the gain in pp if the ctb user were \
     to get a score with the given pp value"
)]
#[usage("[username] [number]")]
#[example("badewanne3 321.98")]
#[aliases("wic")]
pub async fn whatifctb(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    whatif_main(GameMode::CTB, ctx, msg, args).await
}

pub enum WhatIfData {
    NonTop100,
    NoScores {
        rank: Option<u32>,
    },
    Top100 {
        bonus_pp: f32,
        new_pp: f32,
        new_pos: usize,
        max_pp: f32,
        rank: Option<u32>,
    },
}
