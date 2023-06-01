use std::{collections::HashMap, sync::Arc};

use bathbot_macros::SlashCommand;
use bathbot_psql::model::osu::MapBookmark;
use bathbot_util::{constants::GENERAL_ISSUE, MessageOrigin};
use eyre::Result;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    active::{impls::BookmarksPagination, ActiveMessages},
    core::Context,
    util::{interaction::InteractionCommand, Authored, InteractionCommandExt},
};

#[derive(CreateCommand, CommandModel, SlashCommand)]
#[command(
    name = "bookmarks",
    desc = "List all your bookmarked maps",
    help = "List all your bookmarked maps.\n\
    You can bookmark maps by rightclicking a bot message containing a map, \
    \"Apps\", and then click on \"Bookmark map\"."
)]
pub struct Bookmarks {
    #[command(desc = "Choose how the maps should be ordered")]
    sort: Option<BookmarksSort>,
}

#[derive(Copy, Clone, CommandOption, CreateOption)]
pub enum BookmarksSort {
    #[option(name = "Bookmark date", value = "bookmark_date")]
    BookmarkDate,
    #[option(name = "Artist", value = "artist")]
    Artist,
    #[option(name = "Title", value = "title")]
    Title,
    #[option(name = "AR", value = "ar")]
    Ar,
    #[option(name = "CS", value = "cs")]
    Cs,
    #[option(name = "HP", value = "hp")]
    Hp,
    #[option(name = "OD", value = "od")]
    Od,
    #[option(name = "Length", value = "len")]
    Length,
}

impl Default for BookmarksSort {
    fn default() -> Self {
        Self::BookmarkDate
    }
}

pub async fn slash_bookmarks(ctx: Arc<Context>, mut command: InteractionCommand) -> Result<()> {
    let args = Bookmarks::from_interaction(command.input_data())?;
    let owner = command.user_id()?;

    let mut bookmarks = match ctx.bookmarks().get(owner).await {
        Ok(bookmarks) => bookmarks,
        Err(err) => {
            let _ = command.error(&ctx, GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    process_bookmarks(&mut bookmarks, args);

    let origin = MessageOrigin::new(command.guild_id(), command.channel_id());

    let pagination = BookmarksPagination::builder()
        .bookmarks(bookmarks)
        .origin(origin)
        .cached_entries(HashMap::default())
        .defer_next(false)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(ctx, &mut command)
        .await
}

fn process_bookmarks(bookmarks: &mut [MapBookmark], args: Bookmarks) {
    match args.sort.unwrap_or_default() {
        BookmarksSort::BookmarkDate => {
            // Sorted by database
        }
        BookmarksSort::Artist => bookmarks.sort_unstable_by(|a, b| {
            a.artist
                .cmp(&b.artist)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Title => bookmarks.sort_unstable_by(|a, b| {
            a.title
                .cmp(&b.title)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Ar => bookmarks.sort_unstable_by(|a, b| {
            a.ar.total_cmp(&b.ar)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Cs => bookmarks.sort_unstable_by(|a, b| {
            a.cs.total_cmp(&b.cs)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Hp => bookmarks.sort_unstable_by(|a, b| {
            a.hp.total_cmp(&b.hp)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Od => bookmarks.sort_unstable_by(|a, b| {
            a.od.total_cmp(&b.od)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Length => bookmarks.sort_unstable_by(|a, b| {
            a.seconds_drain
                .cmp(&b.seconds_drain)
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
    }
}
