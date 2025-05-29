use std::{collections::HashMap, fmt::Write};

use bathbot_macros::SlashCommand;
use bathbot_model::command_fields::GameModeOption;
use bathbot_psql::model::osu::MapBookmark;
use bathbot_util::{
    Authored, CowUtils, MessageOrigin,
    constants::GENERAL_ISSUE,
    query::{BookmarkCriteria, FilterCriteria, IFilterCriteria},
};
use eyre::Result;
use rosu_v2::prelude::GameMode;
use twilight_interactions::command::{CommandModel, CommandOption, CreateCommand, CreateOption};

use crate::{
    active::{ActiveMessages, impls::BookmarksPagination},
    core::Context,
    util::{InteractionCommandExt, interaction::InteractionCommand},
};

#[derive(CreateCommand, CommandModel, SlashCommand)]
#[command(
    name = "bookmarks",
    desc = "List all your bookmarked maps",
    help = "List all your bookmarked maps. You can bookmark maps by:\n\
    1. Rightclicking a bot message that contains a single map\n\
    2. Click on `Apps`\n\
    3. Click on `Bookmark map`."
)]
#[flags(EPHEMERAL)]
pub struct Bookmarks {
    #[command(desc = "Choose how the maps should be ordered")]
    sort: Option<BookmarksSort>,
    #[command(
        desc = "Specify a search query containing artist, AR, BPM, language, ...",
        help = "Filter out maps similarly as you filter maps in osu! itself.\n\
        You can specify the artist, difficulty, title, language, genre or limit values for \
        ar, cs, hp, od, bpm, length, bookmarked, or rankeddate.\n\
        Example: `od>=9 od<9.5 len>180 difficulty=insane bookmarked<2020-12-31 genre=electronic`"
    )]
    query: Option<String>,
    #[command(desc = "Filter out maps that don't belong to a gamemode")]
    mode: Option<GameModeOption>,
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

pub async fn slash_bookmarks(mut command: InteractionCommand) -> Result<()> {
    let args = Bookmarks::from_interaction(command.input_data())?;
    let owner = command.user_id()?;

    let mut bookmarks = match Context::bookmarks().get(owner).await {
        Ok(bookmarks) => bookmarks,
        Err(err) => {
            let _ = command.error(GENERAL_ISSUE).await?;

            return Err(err);
        }
    };

    let criteria = args.query.as_deref().map(BookmarkCriteria::create);

    process_bookmarks(&mut bookmarks, &args, criteria.as_ref());
    let content = msg_content(&args, criteria.as_ref());
    let filtered = criteria.is_some() || args.mode.is_some();

    let origin = MessageOrigin::new(command.guild_id(), command.channel_id());

    let pagination = BookmarksPagination::builder()
        .bookmarks(bookmarks)
        .origin(origin)
        .cached_entries(HashMap::default())
        .filtered_maps(Some(filtered))
        .defer_next(false)
        .token(command.token.clone())
        .content(content)
        .msg_owner(owner)
        .build();

    ActiveMessages::builder(pagination)
        .start_by_update(true)
        .begin(&mut command)
        .await
}

fn process_bookmarks(
    bookmarks: &mut Vec<MapBookmark>,
    args: &Bookmarks,
    criteria: Option<&FilterCriteria<BookmarkCriteria<'_>>>,
) {
    if let Some(mode) = args.mode.map(GameMode::from) {
        bookmarks.retain(|bookmark| bookmark.mode == mode);
    }

    if let Some(criteria) = criteria {
        bookmarks.retain(|bookmark| {
            let mut matches = true;

            matches &= criteria.ar.contains(bookmark.ar);
            matches &= criteria.cs.contains(bookmark.cs);
            matches &= criteria.hp.contains(bookmark.hp);
            matches &= criteria.od.contains(bookmark.od);
            matches &= criteria.length.contains(bookmark.seconds_drain as f32);
            matches &= criteria.bpm.contains(bookmark.bpm);

            matches &= criteria.insert_date.contains(bookmark.insert_date.date());
            matches &= bookmark
                .ranked_date
                .is_some_and(|datetime| criteria.ranked_date.contains(datetime.date()));

            let version = bookmark.version.cow_to_ascii_lowercase();
            matches &= criteria.version.matches(&version);

            let artist = bookmark.artist.cow_to_ascii_lowercase();
            matches &= criteria.artist.matches(&artist);

            let title = bookmark.title.cow_to_ascii_lowercase();
            matches &= criteria.title.matches(&title);

            let language = format!("{:?}", bookmark.language).to_lowercase();
            matches &= criteria.language.matches(&language);

            let genre = format!("{:?}", bookmark.genre).to_lowercase();
            matches &= criteria.genre.matches(&genre);

            if matches && criteria.has_search_terms() {
                let terms = [
                    artist.as_ref(),
                    title.as_ref(),
                    version.as_ref(),
                    language.as_str(),
                    genre.as_str(),
                ];

                matches &= criteria
                    .search_terms()
                    .all(|term| terms.iter().any(|searchable| searchable.contains(term)))
            }

            matches
        });
    }

    match args.sort.unwrap_or_default() {
        BookmarksSort::BookmarkDate => {
            // Sorted by database
        }
        BookmarksSort::Artist => bookmarks.sort_unstable_by(|a, b| {
            a.artist
                .cow_to_ascii_lowercase()
                .cmp(&b.artist.cow_to_ascii_lowercase())
                .then_with(|| b.insert_date.cmp(&a.insert_date))
        }),
        BookmarksSort::Title => bookmarks.sort_unstable_by(|a, b| {
            a.title
                .cow_to_ascii_lowercase()
                .cmp(&b.title.cow_to_ascii_lowercase())
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

fn msg_content(
    args: &Bookmarks,
    criteria: Option<&FilterCriteria<BookmarkCriteria<'_>>>,
) -> String {
    let mut content = String::new();

    if let Some(mode) = args.mode.map(GameMode::from) {
        let _ = write!(
            content,
            "`Mode: {}`",
            match mode {
                GameMode::Osu => "osu!",
                GameMode::Taiko => "Taiko",
                GameMode::Catch => "Catch",
                GameMode::Mania => "Mania",
            }
        );
    }

    if let Some(criteria) = criteria {
        criteria.display(&mut content);
    }

    content
}
