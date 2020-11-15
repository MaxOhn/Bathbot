use crate::{
    arguments::{Args, NameArgs},
    custom_client::{OsuMedal, OsuMedalGroup},
    embeds::{EmbedData, MedalsMissingEmbed},
    pagination::{MedalsMissingPagination, Pagination},
    util::{
        constants::{OSU_API_ISSUE, OSU_WEB_ISSUE},
        numbers, MessageExt,
    },
    BotResult, Context,
};

use rosu::model::GameMode;
use std::{cmp::Ordering, collections::HashSet, sync::Arc};
use twilight_model::channel::Message;

const GROUPS: [OsuMedalGroup; 7] = [
    OsuMedalGroup::Skill,
    OsuMedalGroup::Dedication,
    OsuMedalGroup::HushHush,
    OsuMedalGroup::BeatmapPacks,
    OsuMedalGroup::SeasonalSpotlights,
    OsuMedalGroup::BeatmapSpotlights,
    OsuMedalGroup::ModIntroduction,
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
    let user = match ctx.osu().user(name.as_str()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let content = format!("User `{}` was not found", name);
            return msg.error(&ctx, content).await;
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let profile_fut = ctx
        .clients
        .custom
        .get_osu_profile(user.user_id, GameMode::STD, true);
    let (mut profile, medals) = match profile_fut.await {
        Ok(tuple) => tuple,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_WEB_ISSUE).await;
            return Err(why.into());
        }
    };
    let medal_count = (medals.len() - profile.medals.len(), medals.len());
    let owned: HashSet<_> = profile
        .medals
        .drain(..)
        .map(|medal| medal.medal_id)
        .collect();
    let mut medals: Vec<_> = medals
        .into_iter()
        .filter(|(id, _)| !owned.contains(id))
        .map(|(_, medal)| MedalType::Medal(medal))
        .collect();
    medals.extend(GROUPS.iter().copied().map(MedalType::Group));
    medals.sort_unstable();
    let limit = medals.len().min(15);
    let pages = numbers::div_euclid(15, medals.len());
    let data = MedalsMissingEmbed::new(
        &profile,
        &medals[..limit],
        medal_count,
        limit == medals.len(),
        (1, pages),
    );

    // Send the embed
    let embed = data.build().build()?;
    let response = ctx
        .http
        .create_message(msg.channel_id)
        .embed(embed)?
        .await?;

    // Skip pagination if too few entries
    if medals.len() <= 15 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = MedalsMissingPagination::new(response, profile, medals, medal_count);
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            warn!("Pagination error (medals missing): {}", why)
        }
    });
    Ok(())
}

pub enum MedalType {
    Group(OsuMedalGroup),
    Medal(OsuMedal),
}

impl MedalType {
    fn group(&self) -> &OsuMedalGroup {
        match self {
            Self::Group(g) => g,
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
