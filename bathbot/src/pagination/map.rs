use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMods};
use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::CustomAttrs,
    embeds::{EmbedData, MapEmbed},
};

use super::{Context, Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct MapPagination {
    mapset: Beatmapset,
    maps: Vec<Beatmap>,
    mods: GameMods,
    attrs: CustomAttrs,
}

impl MapPagination {
    pub fn builder(
        mapset: Beatmapset,
        maps: Vec<Beatmap>,
        mods: GameMods,
        start_idx: usize,
        attrs: CustomAttrs,
    ) -> PaginationBuilder {
        let mut pages = Pages::new(1, maps.len());
        pages.index = start_idx;

        let pagination = Self {
            mapset,
            maps,
            mods,
            attrs,
        };

        let kind = PaginationKind::Map(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let map = &self.maps[pages.index];

        MapEmbed::new(map, &self.mapset, self.mods, &self.attrs, ctx, pages)
            .await
            .map(EmbedData::build)
            .wrap_err("failed to create embed data")
    }
}
