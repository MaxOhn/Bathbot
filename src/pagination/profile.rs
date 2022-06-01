use twilight_model::channel::embed::Embed;

use crate::{
    commands::osu::{ProfileData, ProfileSize},
    core::Context,
    embeds::{EmbedData, ProfileEmbed},
};

use super::{Pages, PaginationBuilder, PaginationKind};

// Not using #[pagination(...)] since it requires special initialization
pub struct ProfilePagination {
    curr_size: ProfileSize,
    data: ProfileData,
}

impl ProfilePagination {
    pub fn builder(curr_size: ProfileSize, data: ProfileData) -> PaginationBuilder {
        let mut pages = Pages::new(1, 3);
        pages.index = curr_size as usize;

        let pagination = Self { curr_size, data };

        let kind = PaginationKind::Profile(Box::new(pagination));

        PaginationBuilder::new(kind, pages)
    }

    pub async fn build_page(&mut self, ctx: &Context, pages: &Pages) -> Embed {
        self.curr_size = match pages.index {
            0 => ProfileSize::Compact,
            1 => ProfileSize::Medium,
            2 => ProfileSize::Full,
            _ => unreachable!(),
        };

        ProfileEmbed::get_or_create(ctx, self.curr_size, &mut self.data)
            .await
            .to_owned()
            .build()
    }
}
