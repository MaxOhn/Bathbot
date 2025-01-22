use std::{cmp::Reverse, collections::HashMap};

use bathbot_macros::command;
use bathbot_util::{
    constants::{GENERAL_ISSUE, },
    matcher, IntHasher, MessageBuilder,
};
use eyre::{Report, Result};
use rosu_v2::{model::GameMode, prelude::OsuError, request::UserId};
use time::{Duration, OffsetDateTime};

use super::{SnipeGameMode, SnipePlayerGain, SnipePlayerLoss};
use crate::{
    active::{impls::SnipeDifferencePagination, ActiveMessages},
    core::commands::{prefix::Args, CommandOrigin},
    manager::redis::{
        osu::{UserArgs, UserArgsError},
        
    },
    Context,
};

#[command]
#[desc("Display a user's recently acquired national #1 scores")]
#[help(
    "Display a user's national #1 scores that they acquired within the last week.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("sg", "snipegain", "snipesgain")]
#[group(Osu)]
async fn prefix_snipedgain(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerGain::args(args, None);

    player_gain(msg.into(), args).await
}

#[command]
#[desc("Display a user's recently acquired national #1 ctb scores")]
#[help(
    "Display a user's national #1 ctb scores that they acquired within the last week.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "sgc",
    "snipedgaincatch",
    "snipegainctb",
    "snipegaincatch",
    "snipesgainctb",
    "snipesgaincatch"
)]
#[group(Catch)]
async fn prefix_snipedgainctb(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerGain::args(args, Some(GameMode::Catch));

    player_gain(msg.into(), args).await
}

#[command]
#[desc("Display a user's recently acquired national #1 mania scores")]
#[help(
    "Display a user's national #1 mania scores that they acquired within the last week.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("sgm", "snipegainmania", "snipesgainmania")]
#[group(Mania)]
async fn prefix_snipedgainmania(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerGain::args(args, Some(GameMode::Mania));

    player_gain(msg.into(), args).await
}

#[command]
#[desc("Display a user's recently lost national #1 scores")]
#[help(
    "Display a user's national #1 scores that they lost within the last week.\n\
    Data for osu!standard originates from [Mr Helix](https://osu.ppy.sh/users/2330619)'s \
    [huismetbenen](https://snipe.huismetbenen.nl/)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "sl",
    "snipeloss",
    "snipesloss",
    "snipedlost",
    "snipelost",
    "snipeslost"
)]
#[group(Osu)]
async fn prefix_snipedloss(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerLoss::args(args, None);

    player_loss(msg.into(), args).await
}

#[command]
#[desc("Display a user's recently lost national #1 ctb scores")]
#[help(
    "Display a user's national #1 ctb scores that they lost within the last week.\n\
    Data for osu!catch originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "slc",
    "snipelossctb",
    "snipelosscatch",
    "snipeslossctb",
    "snipeslosscatch",
    "snipedlostctb",
    "snipedlostcatch",
    "snipelostctb",
    "snipelostcatch",
    "snipeslostctb",
    "snipeslostcatch"
)]
#[group(Catch)]
async fn prefix_snipedlossctb(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerLoss::args(args, Some(GameMode::Catch));

    player_loss(msg.into(), args).await
}

#[command]
#[desc("Display a user's recently lost national #1 mania scores")]
#[help(
    "Display a user's national #1 mania scores that they lost within the last week.\n\
    Data for osu!mania originates from [molneya](https://osu.ppy.sh/users/8945180)'s \
    [kittenroleplay](https://snipes.kittenroleplay.com)."
)]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases(
    "slm",
    "snipelossmania",
    "snipeslossmania",
    "snipedlostmania",
    "snipelostmania",
    "snipeslostmania"
)]
#[group(Mania)]
async fn prefix_snipedlossmania(msg: &Message, args: Args<'_>) -> Result<()> {
    let args = SnipePlayerLoss::args(args, Some(GameMode::Mania));

    player_loss(msg.into(), args).await
}

pub(super) async fn player_gain(orig: CommandOrigin<'_>, args: SnipePlayerGain<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(orig, args);

    sniped_diff(orig, Difference::Gain, user_id, mode).await
}

pub(super) async fn player_loss(orig: CommandOrigin<'_>, args: SnipePlayerLoss<'_>) -> Result<()> {
    let (user_id, mode) = user_id_mode!(orig, args);

    sniped_diff(orig, Difference::Loss, user_id, mode).await
}

async fn sniped_diff(
    orig: CommandOrigin<'_>,
    diff: Difference,
    user_id: UserId,
    mode: GameMode,
) -> Result<()> {
    let owner = orig.user_id()?;

    // Request the user
    let user_args = UserArgs::rosu_id(&user_id, mode).await;

    let user = match Context::redis().osu_user(user_args).await {
        Ok(user) => user,
        Err(UserArgsError::Osu(OsuError::NotFound)) => {
            let content = match user_id {
                UserId::Id(user_id) => format!("User with id {user_id} was not found"),
                UserId::Name(name) => format!("User `{name}` was not found"),
            };

            return orig.error(content).await;
        }
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;
            let err = Report::new(err).wrap_err("Failed to get user");

            return Err(err);
        }
    };

    let country_code = user.country_code.as_str();
    let username = user.username.as_str();
    let user_id = user.user_id.to_native();

    if !Context::huismetbenen()
        .is_supported(country_code, mode)
        .await
    {
        let content = format!("`{username}`'s country {country_code} is not supported :(");

        return orig.error(content).await;
    }

    let client = Context::client();
    let now = OffsetDateTime::now_utc();
    let week_ago = now - Duration::weeks(1);

    // Request the scores
    let scores_fut = match diff {
        Difference::Gain => client.get_national_snipes(user_id, true, week_ago, mode),
        Difference::Loss => client.get_national_snipes(user_id, false, week_ago, mode),
    };

    let mut scores = match scores_fut.await {
        Ok(scores) => scores,
        Err(err) => {
            let _ = orig.error(GENERAL_ISSUE).await;

            return Err(err.wrap_err("failed to get snipes"));
        }
    };

    if scores.is_empty() {
        let content = format!(
            "`{username}` didn't {diff} national {mode} #1s in the last week.",
            diff = match diff {
                Difference::Gain => "gain any new",
                Difference::Loss => "lose any",
            },
            mode = match mode {
                GameMode::Osu => "osu!standard",
                GameMode::Taiko => "osu!taiko",
                GameMode::Catch => "osu!catch",
                GameMode::Mania => "osu!mania",
            }
        );

        let builder = MessageBuilder::new().embed(content);
        orig.create_message(builder).await?;

        return Ok(());
    }

    scores.sort_unstable_by_key(|s| Reverse(s.date));

    let pagination = SnipeDifferencePagination::builder()
        .user(user)
        .diff(diff)
        .scores(scores.into_boxed_slice())
        .star_map(HashMap::with_hasher(IntHasher))
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(orig)
        .await
}

#[derive(Copy, Clone)]
pub enum Difference {
    Gain,
    Loss,
}

impl<'m> SnipePlayerGain<'m> {
    fn args(mut args: Args<'m>, mode: Option<GameMode>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self {
            mode: mode.and_then(SnipeGameMode::try_from_mode),
            name,
            discord,
        }
    }
}

impl<'m> SnipePlayerLoss<'m> {
    fn args(mut args: Args<'m>, mode: Option<GameMode>) -> Self {
        let mut name = None;
        let mut discord = None;

        if let Some(arg) = args.next() {
            match matcher::get_mention_user(arg) {
                Some(id) => discord = Some(id),
                None => name = Some(arg.into()),
            }
        }

        Self {
            mode: mode.and_then(SnipeGameMode::try_from_mode),
            name,
            discord,
        }
    }
}
