use eyre::{Result, WrapErr};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameModsIntermode};
use twilight_model::channel::message::embed::Embed;

use crate::{
    commands::osu::CustomAttrs,
    embeds::{EmbedData, MapEmbed, MessageOrigin},
};

use super::{Context, Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct MapPagination {
    mapset: Beatmapset,
    maps: Vec<Beatmap>,
    mods: GameModsIntermode,
    attrs: CustomAttrs,
    origin: MessageOrigin,
}

impl MapPagination {
    pub fn builder(
        mapset: Beatmapset,
        maps: Vec<Beatmap>,
        mods: GameModsIntermode,
        start_idx: usize,
        attrs: CustomAttrs,
        origin: MessageOrigin,
    ) -> PaginationBuilder {
        let mut pages = Pages::new(1, maps.len());
        pages.update(|_| start_idx);

        let pagination = Self {
            mapset,
            maps,
            mods,
            attrs,
            origin,
        };

        let kind = PaginationKind::Map(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Result<Embed> {
        let map = &self.maps[pages.index()];

        let embed_fut = MapEmbed::new(
            map,
            &self.mapset,
            &self.mods,
            &self.attrs,
            self.origin,
            ctx,
            pages,
        );

        embed_fut
            .await
            .map(EmbedData::build)
            .wrap_err("failed to create embed data")
    }
}
