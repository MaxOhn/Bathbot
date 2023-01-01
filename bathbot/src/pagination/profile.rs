use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{ProfileData, ProfileKind},
    core::Context,
    embeds::{EmbedData, ProfileEmbed},
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct ProfilePagination {
    kind: ProfileKind,
    data: ProfileData,
}

impl ProfilePagination {
    pub fn builder(curr_kind: ProfileKind, data: ProfileData) -> PaginationBuilder {
        let mut pages = Pages::new(1, 3);
        pages.index = curr_kind as usize;

        let pagination = Self {
            kind: curr_kind,
            data,
        };

        let kind = PaginationKind::Profile(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        self.kind = match pages.index {
            0 => ProfileKind::Compact,
            1 => ProfileKind::UserStats,
            2 => ProfileKind::Top100Stats,
            3 => ProfileKind::Top100Mods,
            4 => ProfileKind::Top100Mappers,
            5 => ProfileKind::MapperStats,
            _ => unreachable!(),
        };

        ProfileEmbed::new(ctx, self.kind, &mut self.data)
            .await
            .build()
    }
}