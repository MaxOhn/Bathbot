use std::fmt::{Display, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{RankAccPeaks, RelaxPlayersDataResponse};
use bathbot_util::{
    constants::RELAX,
    datetime::{HowLongAgoText, SecToMinSec, NAIVE_DATETIME_FORMAT},
    fields,
    numbers::{round, MinMaxAvg, Number, WithComma},
    osu::flag_url,
    osu::BonusPP,
    AuthorBuilder, EmbedBuilder, FooterBuilder, MessageOrigin,
};
use eyre::Result;
use futures::future::BoxFuture;
use rkyv::rancor::{Panic, ResultExt};
use rosu_v2::prelude::{
    GameModIntermode, GameMode, GameModsIntermode, Grade, Score,
    UserHighestRank as RosuUserHighestRank, UserKudosu,
};
use time::UtcOffset;
use twilight_model::{
    channel::message::{
        component::{ActionRow, SelectMenu, SelectMenuOption},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::osu::ProfileKind,
    manager::redis::osu::CachedUser,
    util::{
        interaction::InteractionComponent, osu::grade_emote, Authored, CachedUserExt, ComponentExt,
        Emote,
    },
};
pub struct RelaxProfile {
    user: CachedUser,
    discord_id: Option<Id<UserMarker>>,
    tz: Option<UtcOffset>,
    info: RelaxPlayersDataResponse,
    origin: MessageOrigin,
    msg_owner: Id<UserMarker>,
}

// impl IActiveMessage for RelaxProfile {
//     fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
//         Box::pin(self.compact())
//     }
// }

impl RelaxProfile {
    pub fn new(
        user: CachedUser,
        discord_id: Option<Id<UserMarker>>,
        tz: Option<UtcOffset>,
        info: RelaxPlayersDataResponse,
        origin: MessageOrigin,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            user,
            discord_id,
            tz,
            info,
            origin,
            msg_owner,
        }
    }

    pub fn compact(&mut self) -> Result<EmbedBuilder> {
        let user_id = self.user.user_id.to_native();
        let stats = &self.info;
        let mut description = format!(
            "Accuracy: [`{acc:.2}%`]({origin} \"{acc}\")\n\
            Playcount: `{playcount}`",
            acc = stats.total_accuracy.unwrap_or_default(),
            origin = self.origin,
            playcount = WithComma::new(stats.playcount),
        );

        let embed = EmbedBuilder::new()
            .author(self.author_builder())
            .description(description)
            .thumbnail(self.user.avatar_url.as_ref());

        Ok(embed)
    }

    fn author_builder(&self) -> AuthorBuilder {
        let country_code = self.user.country_code.as_str();
        let pp = self.info.total_pp;

        let text = format!(
            "{name}: {pp}pp (#{rank}, {country_code}{country_rank})",
            name = self.user.username,
            pp = WithComma::new(pp.unwrap()),
            rank = self.info.rank.unwrap_or_default(),
            country_rank = self.info.country_rank.unwrap_or_default(),
        );

        let url = format!("{RELAX}/users/{}", self.user.user_id);
        let icon = flag_url(country_code);
        AuthorBuilder::new(text).url(url).icon_url(icon)
    }
}
