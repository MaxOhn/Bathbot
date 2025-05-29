use bathbot_util::{
    CowUtils,
    query::{FilterCriteria, RegularCriteria, Searchable},
};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone)]
pub struct DbBeatmap {
    pub map_id: i32,
    pub mapset_id: i32,
    pub user_id: i32,
    pub map_version: String,
    pub seconds_drain: i32,
    pub count_circles: i32,
    pub count_sliders: i32,
    pub count_spinners: i32,
    pub bpm: f32,
}

impl Searchable<RegularCriteria<'_>> for DbBeatmap {
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
        let mut matches = true;

        matches &= criteria.length.contains(self.seconds_drain as f32);
        matches &= criteria.bpm.contains(self.bpm);

        if matches && criteria.has_search_terms() {
            let version = self.map_version.cow_to_ascii_lowercase();

            matches &= criteria.search_terms().any(|term| version.contains(term));
        }

        matches
    }
}

#[derive(Debug)]
pub enum DbMapContent {
    Present(Vec<u8>),
    ChecksumMismatch,
    Missing,
}

#[derive(Archive, Deserialize, Serialize)]
pub struct MapVersion {
    pub map_id: i32,
    pub version: String,
}
