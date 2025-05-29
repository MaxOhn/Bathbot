use bathbot_util::{
    CowUtils,
    query::{FilterCriteria, RegularCriteria, Searchable},
};
use time::OffsetDateTime;

#[derive(Clone)]
pub struct DbBeatmapset {
    pub mapset_id: i32,
    pub user_id: i32,
    pub artist: String,
    pub title: String,
    pub creator: String,
    pub rank_status: i16,
    pub ranked_date: Option<OffsetDateTime>,
    pub thumbnail: String,
    pub cover: String,
}

impl Searchable<RegularCriteria<'_>> for DbBeatmapset {
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
        let mut matches = true;

        let artist = self.artist.cow_to_ascii_lowercase();
        let creator = self.creator.cow_to_ascii_lowercase();
        let title = self.title.cow_to_ascii_lowercase();

        matches &= criteria.artist.matches(artist.as_ref());
        matches &= criteria.creator.matches(creator.as_ref());
        matches &= criteria.title.matches(title.as_ref());

        if matches && criteria.has_search_terms() {
            let terms = [artist, creator, title];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)));
        }

        matches
    }
}

pub struct ArtistTitle {
    pub artist: String,
    pub title: String,
}
