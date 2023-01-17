use twilight_model::channel::embed::Embed;

use crate::{
    embeds::{EmbedData, SimulateData, SimulateEmbed},
    manager::OsuMap,
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct SimulatePagination {
    map: OsuMap,
    pub simulate_data: SimulateData,
}

impl SimulatePagination {
    pub fn builder(map: OsuMap, simulate_data: SimulateData) -> PaginationBuilder {
        // initialization doesn't really matter since the index is always set manually anyway
        let pages = Pages::new(1, usize::MAX);

        let pagination = Self { map, simulate_data };
        let kind = PaginationKind::Simulate(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub fn build_page(&mut self) -> Embed {
        SimulateEmbed::new(&self.map, &self.simulate_data).build()
    }
}
