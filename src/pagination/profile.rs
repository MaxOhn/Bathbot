use std::sync::Arc;

use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{ProfileData, ProfileSize},
    core::Context,
    embeds::{EmbedData, ProfileEmbed},
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct ProfilePagination {
    ctx: Arc<Context>,
    curr_size: ProfileSize,
    data: ProfileData,
}

impl ProfilePagination {
    pub fn builder(
        ctx: Arc<Context>,
        curr_size: ProfileSize,
        data: ProfileData,
    ) -> PaginationBuilder {
        let mut pages = Pages::new(1, 3);
        pages.index = curr_size as usize;

        let pagination = Self {
            ctx,
            curr_size,
            data,
        };

        let kind = PaginationKind::Profile(pagination);

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, pages: &Pages) -> Embed {
        self.curr_size = match pages.index {
            0 => ProfileSize::Compact,
            1 => ProfileSize::Medium,
            2 => ProfileSize::Full,
            _ => unreachable!(),
        };

        ProfileEmbed::get_or_create(&self.ctx, self.curr_size, &mut self.data)
            .await
            .to_owned()
            .build()
    }
}
