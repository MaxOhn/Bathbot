use crate::{
    custom_client::{OsekaiGrouping, OsekaiMedal, MEDAL_GROUPS},
    database::OsuData,
    embeds::{EmbedData, MedalsMissingEmbed},
    pagination::{MedalsMissingPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        numbers, MessageExt,
    },
    Args, BotResult, CommandData, Context, Name,
};

use hashbrown::HashSet;
use rosu_v2::prelude::OsuError;
use std::{cmp::Ordering, sync::Arc};

#[command]
#[short_desc("Display a list of medals that a user is missing")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mm", "missingmedals")]
async fn medalsmissing(ctx: Arc<Context>, data: CommandData) -> BotResult<()> {
    match data {
        CommandData::Message { msg, mut args, num } => {
            let name = match args.next() {
                Some(arg) => match Args::check_user_mention(&ctx, arg).await {
                    Ok(Ok(osu)) => Some(osu.into_username()),
                    Ok(Err(content)) => return msg.error(&ctx, content).await,
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
                None => match ctx.psql().get_user_osu(msg.author.id).await {
                    Ok(osu) => osu.map(OsuData::into_username),
                    Err(why) => {
                        let _ = msg.error(&ctx, GENERAL_ISSUE).await;

                        return Err(why);
                    }
                },
            };

            _medalsmissing(ctx, CommandData::Message { msg, args, num }, name).await
        }
        CommandData::Interaction { command } => super::slash_medal(ctx, *command).await,
    }
}

pub(super) async fn _medalsmissing(
    ctx: Arc<Context>,
    data: CommandData<'_>,
    name: Option<Name>,
) -> BotResult<()> {
    let name = match name {
        Some(name) => name,
        None => return super::require_link(&ctx, &data).await,
    };

    let user_fut = super::request_user(&ctx, &name, None);
    let medals_fut = ctx.psql().get_medals();

    let (user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{}` was not found", name);

            return data.error(&ctx, content).await;
        }
        (_, Err(why)) => {
            let _ = data.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
        (Err(why), _) => {
            let _ = data.error(&ctx, OSU_API_ISSUE).await;

            return Err(why.into());
        }
    };

    let medals = user.medals.as_ref().unwrap();
    let medal_count = (all_medals.len() - medals.len(), all_medals.len());
    let owned: HashSet<_> = medals.iter().map(|medal| medal.medal_id).collect();

    let mut medals: Vec<_> = all_medals
        .into_iter()
        .filter(|(id, _)| !owned.contains(id))
        .map(|(_, medal)| MedalType::Medal(medal))
        .collect();

    medals.extend(MEDAL_GROUPS.iter().copied().map(MedalType::Group));
    medals.sort_unstable();

    let limit = medals.len().min(15);
    let pages = numbers::div_euclid(15, medals.len());

    let embed_data = MedalsMissingEmbed::new(
        &user,
        &medals[..limit],
        medal_count,
        limit == medals.len(),
        (1, pages),
    );

    // Send the embed
    let builder = embed_data.into_builder().build().into();
    let response_raw = data.create_message(&ctx, builder).await?;

    // Skip pagination if too few entries
    if medals.len() <= 15 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination = MedalsMissingPagination::new(response, user, medals, medal_count);
    let owner = data.author()?.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (medals missing): {}")
        }
    });

    Ok(())
}

pub enum MedalType {
    Group(OsekaiGrouping<'static>),
    Medal(OsekaiMedal),
}

impl MedalType {
    fn group(&self) -> OsekaiGrouping {
        match self {
            Self::Group(g) => *g,
            Self::Medal(m) => OsekaiGrouping(&m.grouping),
        }
    }
}

impl PartialEq for MedalType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MedalType::Group(a), MedalType::Group(b)) => a == b,
            (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id == b.medal_id,
            _ => false,
        }
    }
}

impl Eq for MedalType {}

impl PartialOrd for MedalType {
    fn partial_cmp(&self, other: &MedalType) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MedalType {
    fn cmp(&self, other: &MedalType) -> Ordering {
        self.group()
            .cmp(&other.group())
            .then_with(|| match (self, other) {
                (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id.cmp(&b.medal_id),
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                _ => unreachable!(),
            })
    }
}
