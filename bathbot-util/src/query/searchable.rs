use rosu_v2::prelude::{BeatmapExtended, BeatmapsetExtended, GameMode};

use super::{FilterCriteria, RegularCriteria};
use crate::CowUtils;

pub trait Searchable<F> {
    fn matches(&self, criteria: &FilterCriteria<F>) -> bool;
}

impl Searchable<RegularCriteria<'_>> for BeatmapExtended {
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
        let mut matches = true;

        matches &= criteria.stars.contains(self.stars);
        matches &= criteria.ar.contains(self.ar);
        matches &= criteria.cs.contains(self.cs);
        matches &= criteria.hp.contains(self.hp);
        matches &= criteria.od.contains(self.od);
        matches &= criteria.length.contains(self.seconds_drain as f32);
        matches &= criteria.bpm.contains(self.bpm);
        matches &= self.mode != GameMode::Mania || criteria.keys.contains(self.cs);

        if let Some(ref mapset) = self.mapset {
            matches &= mapset.matches(criteria);
        }

        if matches && criteria.has_search_terms() {
            let version = self.version.cow_to_ascii_lowercase();

            matches &= criteria.search_terms().any(|term| version.contains(term));
        }

        matches
    }
}

impl Searchable<RegularCriteria<'_>> for BeatmapsetExtended {
    fn matches(&self, criteria: &FilterCriteria<RegularCriteria<'_>>) -> bool {
        let mut matches = true;

        let artist = self.artist.cow_to_ascii_lowercase();
        let creator = self.creator_name.cow_to_ascii_lowercase();
        let title = self.title.cow_to_ascii_lowercase();

        matches &= criteria.artist.matches(artist.as_ref());
        matches &= criteria.creator.matches(creator.as_ref());
        matches &= criteria.title.matches(title.as_ref());

        if let Some(ref maps) = self.maps {
            matches &= maps.iter().any(|map| map.matches(criteria));
        }

        if matches && criteria.has_search_terms() {
            let terms = [artist, creator, title];

            matches &= criteria.search_terms().all(|term| {
                if terms.iter().any(|searchable| searchable.contains(term)) {
                    true
                } else if let Some(ref maps) = self.maps {
                    maps.iter()
                        .map(|map| map.version.cow_to_ascii_lowercase())
                        .any(|version| version.contains(term))
                } else {
                    false
                }
            });
        }

        matches
    }
}
