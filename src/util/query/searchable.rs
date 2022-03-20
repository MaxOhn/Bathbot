use std::borrow::Cow;

use rosu_pp::Mods;
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Score};

use crate::{
    custom_client::OsuTrackerCountryScore,
    embeds::{calculate_ar, calculate_od},
    util::CowUtils,
};

use super::FilterCriteria;

pub trait Searchable {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool;
}

impl Searchable for Beatmap {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let mut matches = true;

        matches &= criteria.stars.contains(self.stars);
        matches &= criteria.ar.contains(self.ar);
        matches &= criteria.cs.contains(self.cs);
        matches &= criteria.hp.contains(self.hp);
        matches &= criteria.od.contains(self.od);
        matches &= criteria.length.contains(self.seconds_drain as f32);
        matches &= criteria.bpm.contains(self.bpm);
        matches &= self.mode != GameMode::MNA || criteria.keys.contains(self.cs);

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

impl Searchable for Beatmapset {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
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

impl Searchable for Score {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let mut matches = true;

        let mut artist = Cow::default();
        let mut creator = Cow::default();
        let mut title = Cow::default();
        let mut version = Cow::default();

        if let Some(ref map) = self.map {
            let mods = self.mods.bits();
            let clock_rate = mods.clock_rate() as f32;

            let mut ar = map.ar;
            let mut cs = map.cs;
            let mut hp = map.hp;
            let mut od = map.od;

            if mods.hr() {
                ar *= 1.4;
                cs *= 1.3;
                hp *= 1.4;
                od *= 1.4;
            } else if mods.ez() {
                ar *= 0.5;
                cs *= 0.5;
                hp *= 0.5;
                od *= 0.5;
            }

            ar = ar.min(10.0);
            cs = cs.min(10.0);
            hp = hp.min(10.0);
            od = od.min(10.0);

            let len = map.seconds_drain as f32 / clock_rate;

            matches &= criteria.stars.contains(map.stars);
            matches &= criteria.ar.contains(calculate_ar(ar, clock_rate));
            matches &= criteria.cs.contains(cs);
            matches &= criteria.hp.contains(hp);
            matches &= criteria.od.contains(calculate_od(od, clock_rate));
            matches &= criteria.length.contains(len);
            matches &= criteria.bpm.contains(map.bpm * clock_rate);

            let keys = match self.mods.has_key_mod() {
                Some(GameMods::Key1) => 1.0,
                Some(GameMods::Key2) => 2.0,
                Some(GameMods::Key3) => 3.0,
                Some(GameMods::Key4) => 4.0,
                Some(GameMods::Key5) => 5.0,
                Some(GameMods::Key6) => 6.0,
                Some(GameMods::Key7) => 7.0,
                Some(GameMods::Key8) => 8.0,
                Some(GameMods::Key9) => 9.0,
                None => map.cs,
                _ => unreachable!(),
            };

            matches &= map.mode != GameMode::MNA || criteria.keys.contains(keys);

            version = map.version.cow_to_ascii_lowercase();
        }

        if let Some(mapset) = self.mapset.as_ref().filter(|_| matches) {
            artist = mapset.artist.cow_to_ascii_lowercase();
            creator = mapset.creator_name.cow_to_ascii_lowercase();
            title = mapset.title.cow_to_ascii_lowercase();

            matches &= criteria.artist.matches(artist.as_ref());
            matches &= criteria.creator.matches(creator.as_ref());
            matches &= criteria.title.matches(title.as_ref());
        }

        if matches && criteria.has_search_terms() {
            let terms = [artist, creator, version, title];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)));
        }

        matches
    }
}

impl Searchable for OsuTrackerCountryScore {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let mut matches = true;

        let len = self.seconds_total as f32 / self.mods.bits().clock_rate() as f32;
        matches &= criteria.length.contains(len);

        let creator = self.mapper.cow_to_ascii_lowercase();
        matches &= criteria.creator.matches(creator.as_ref());

        if matches && criteria.has_search_terms() {
            let terms = [self.name.cow_to_ascii_lowercase(), creator];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)));
        }

        matches
    }
}
