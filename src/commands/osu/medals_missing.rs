use super::request_user;
use crate::{
    arguments::{Args, NameArgs},
    database::{MedalGroup, OsuMedal},
    embeds::{EmbedData, MedalsMissingEmbed},
    pagination::{MedalsMissingPagination, Pagination},
    util::{
        constants::{GENERAL_ISSUE, OSU_WEB_ISSUE},
        numbers, MessageExt,
    },
    BotResult, Context,
};

use hashbrown::HashSet;
use rosu_v2::prelude::OsuError;
use std::{cmp::Ordering, sync::Arc};
use twilight_model::channel::Message;

const GROUPS: [MedalGroup; 8] = [
    MedalGroup::Skill,
    MedalGroup::Dedication,
    MedalGroup::HushHush,
    MedalGroup::BeatmapPacks,
    MedalGroup::BeatmapChallengePacks,
    MedalGroup::SeasonalSpotlights,
    MedalGroup::BeatmapSpotlights,
    MedalGroup::ModIntroduction,
];

#[command]
#[short_desc("Display a list of medals that a user is missing")]
#[usage("[username]")]
#[example("badewanne3")]
#[aliases("mm", "missingmedals")]
async fn medalsmissing(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = NameArgs::new(&ctx, args);

    let name = match args.name.or_else(|| ctx.get_link(msg.author.id.0)) {
        Some(name) => name,
        None => return super::require_link(&ctx, msg).await,
    };

    let user_fut = request_user(&ctx, &name, None);
    let medals_fut = ctx.psql().get_medals();

    let (user, all_medals) = match tokio::join!(user_fut, medals_fut) {
        (Ok(user), Ok(medals)) => (user, medals),
        (Err(OsuError::NotFound), _) => {
            let content = format!("User `{}` was not found", name);

            return msg.error(&ctx, content).await;
        }
        (_, Err(why)) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;

            return Err(why);
        }
        (Err(why), _) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;

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

    medals.extend(GROUPS.iter().copied().map(MedalType::Group));
    medals.sort_unstable();

    let limit = medals.len().min(15);
    let pages = numbers::div_euclid(15, medals.len());

    let data = MedalsMissingEmbed::new(
        &user,
        &medals[..limit],
        medal_count,
        limit == medals.len(),
        (1, pages),
    );

    // Send the embed
    let embed = data.build().build()?;
    let response = msg.respond_embed(&ctx, embed).await?;

    // Skip pagination if too few entries
    if medals.len() <= 15 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = MedalsMissingPagination::new(response, user, medals, medal_count);
    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (medals missing): {}")
        }
    });

    Ok(())
}

pub enum MedalType {
    Group(MedalGroup),
    Medal(OsuMedal),
}

impl MedalType {
    fn group(&self) -> &MedalGroup {
        match self {
            Self::Group(g) => &g,
            Self::Medal(m) => &m.grouping,
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
            .cmp(other.group())
            .then_with(|| match (self, other) {
                (MedalType::Medal(a), MedalType::Medal(b)) => a.medal_id.cmp(&b.medal_id),
                (MedalType::Group(_), MedalType::Medal(_)) => Ordering::Less,
                (MedalType::Medal(_), MedalType::Group(_)) => Ordering::Greater,
                _ => unreachable!(),
            })
    }
}
