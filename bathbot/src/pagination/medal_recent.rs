use bathbot_model::OsekaiMedal;
use bathbot_util::IntHasher;
use hashbrown::{hash_map::Entry, HashMap};
use rosu_v2::prelude::MedalCompact;
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::MedalAchieved,
    embeds::MedalEmbed,
    manager::redis::{osu::User, RedisData},
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct MedalRecentPagination {
    user: RedisData<User>,
    cached_medals: HashMap<u32, OsekaiMedal, IntHasher>,
    achieved_medals: Vec<MedalCompact>,
    embeds: HashMap<usize, MedalEmbed, IntHasher>,
    medals: Vec<OsekaiMedal>,
}

impl MedalRecentPagination {
    pub fn builder(
        user: RedisData<User>,
        initial_medal: OsekaiMedal,
        achieved_medals: Vec<MedalCompact>,
        index: usize,
        embed_data: MedalEmbed,
        medals: Vec<OsekaiMedal>,
    ) -> PaginationBuilder {
        let mut embeds = HashMap::default();
        embeds.insert(index, embed_data);
        let mut pages = Pages::new(1, achieved_medals.len());
        pages.update(|_| index.saturating_sub(1));
        let mut cached_medals = HashMap::default();
        cached_medals.insert(initial_medal.medal_id, initial_medal);

        let pagination = Self {
            user,
            cached_medals,
            achieved_medals,
            embeds,
            medals,
        };

        let kind = PaginationKind::MedalRecent(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub fn build_page(&mut self, pages: &Pages) -> Embed {
        let idx = pages.index() + 1;

        let embed = match self.embeds.entry(idx) {
            Entry::Occupied(e) => e.get().to_owned(),
            Entry::Vacant(e) => {
                let achieved = &self.achieved_medals[idx - 1];

                let (medal, achieved_at) = match self.cached_medals.get_mut(&achieved.medal_id) {
                    Some(medal) => (medal, achieved.achieved_at),
                    None => match self
                        .medals
                        .iter()
                        .position(|medal| medal.medal_id == achieved.medal_id)
                    {
                        Some(idx) => {
                            let medal = self.medals.swap_remove(idx);

                            let medal = self.cached_medals.entry(medal.medal_id).or_insert(medal);

                            (medal, achieved.achieved_at)
                        }
                        None => panic!("No medal with id `{}`", achieved.medal_id),
                    },
                };

                let achieved = MedalAchieved {
                    user: &self.user,
                    achieved_at,
                    index: idx,
                    medal_count: self.achieved_medals.len(),
                };

                let medal = medal.to_owned();
                let embed_data = MedalEmbed::new(medal, Some(achieved), Vec::new(), None);

                e.insert(embed_data).to_owned()
            }
        };

        embed.minimized()
    }
}
