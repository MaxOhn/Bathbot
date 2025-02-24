use std::collections::{HashMap, hash_map::Entry};

use bathbot_cache::model::CachedArchive;
use bathbot_macros::PaginationBuilder;
use bathbot_model::ArchivedOsekaiMedal;
use bathbot_psql::model::configs::HideSolutions;
use bathbot_util::IntHasher;
use eyre::Result;
use futures::future::{BoxFuture, ready};
use rkyv::vec::ArchivedVec;
use rosu_v2::prelude::MedalCompact;
use twilight_model::{
    channel::message::Component,
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{
        BuildPage, ComponentResult, IActiveMessage,
        pagination::{Pages, handle_pagination_component, handle_pagination_modal},
    },
    commands::osu::{MedalAchieved, MedalEmbed},
    manager::redis::osu::CachedUser,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsRecentPagination {
    user: CachedUser,
    medals: CachedArchive<ArchivedVec<ArchivedOsekaiMedal>>,
    #[pagination(per_page = 1)]
    achieved_medals: Box<[MedalCompact]>,
    embeds: HashMap<usize, MedalEmbed, IntHasher>,
    hide_solutions: HideSolutions,
    content: &'static str,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsRecentPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let idx = self.pages.index();

        let embed = match self.embeds.entry(idx) {
            Entry::Occupied(e) => e.get().to_owned(),
            Entry::Vacant(e) => {
                let achieved = &self.achieved_medals[idx];

                let (medal, achieved_at) = match self
                    .medals
                    .binary_search_by_key(&achieved.medal_id, |medal| medal.medal_id.to_native())
                {
                    Ok(idx) => (&self.medals[idx], achieved.achieved_at),
                    Err(_) => {
                        let err = eyre!("No medal with id {}", achieved.medal_id);

                        return Box::pin(ready(Err(err)));
                    }
                };

                let achieved = MedalAchieved {
                    user: &self.user,
                    achieved_at,
                    index: idx,
                    medal_count: self.achieved_medals.len(),
                };

                let embed_data =
                    MedalEmbed::new(medal, Some(achieved), Vec::new(), None, self.hide_solutions);

                e.insert(embed_data).to_owned()
            }
        };

        BuildPage::new(embed.finish(), false)
            .content(self.content)
            .boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}

impl MedalsRecentPagination {
    pub fn set_index(&mut self, index: usize) {
        self.pages.set_index(index);
    }
}
