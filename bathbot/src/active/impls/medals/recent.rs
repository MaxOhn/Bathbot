use std::collections::{hash_map::Entry, HashMap};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsekaiMedal;
use bathbot_psql::model::configs::HideSolutions;
use bathbot_util::IntHasher;
use eyre::Result;
use futures::future::{ready, BoxFuture};
use rosu_v2::prelude::MedalCompact;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{MedalAchieved, MedalEmbed},
    manager::redis::osu::CachedOsuUser,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsRecentPagination {
    user: CachedOsuUser,
    medals: HashMap<u32, OsekaiMedal, IntHasher>,
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

                let (medal, achieved_at) = match self.medals.get_mut(&achieved.medal_id) {
                    Some(medal) => (medal, achieved.achieved_at),
                    None => {
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

        BuildPage::new(embed.minimized(), false)
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
