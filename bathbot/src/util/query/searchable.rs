use std::borrow::Cow;

use bathbot_model::{OsuTrackerCountryScore, ScoreSlim};
use bathbot_psql::model::osu::{DbBeatmap, DbBeatmapset};
use bathbot_util::CowUtils;
use rosu_pp::{beatmap::BeatmapAttributesBuilder, Beatmap as Map, GameMode as Mode, Mods};
use rosu_v2::prelude::{Beatmap, Beatmapset, GameMode, GameMods, Score};

use crate::{
    commands::osu::{TopEntry, TopIfEntry},
    manager::OsuMap,
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

impl Searchable for DbBeatmap {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
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

impl Searchable for DbBeatmapset {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
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

impl Searchable for Map {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let mut matches = true;

        matches &= criteria.ar.contains(self.ar);
        matches &= criteria.cs.contains(self.cs);
        matches &= criteria.hp.contains(self.hp);
        matches &= criteria.od.contains(self.od);

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
            let mode = match map.mode {
                GameMode::Osu => Mode::Osu,
                GameMode::Taiko => Mode::Taiko,
                GameMode::Catch => Mode::Catch,
                GameMode::Mania => Mode::Mania,
            };

            let attrs = BeatmapAttributesBuilder::default()
                .mode(mode)
                .ar(map.ar)
                .cs(map.cs)
                .hp(map.hp)
                .od(map.od)
                .mods(self.mods.bits())
                .converted(map.convert)
                .build();

            let clock_rate = attrs.clock_rate as f32;
            let len = map.seconds_drain as f32 / clock_rate;

            matches &= criteria.stars.contains(map.stars);
            matches &= criteria.ar.contains(attrs.ar as f32);
            matches &= criteria.cs.contains(attrs.cs as f32);
            matches &= criteria.hp.contains(attrs.hp as f32);
            matches &= criteria.od.contains(attrs.od as f32);
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

            matches &= map.mode != GameMode::Mania || criteria.keys.contains(keys);

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
            let name = self.name.cow_to_ascii_lowercase();

            matches &= criteria
                .search_terms()
                .all(|term| name.contains(term) || creator.contains(term));
        }

        matches
    }
}

impl Searchable for TopIfEntry {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let Self {
            score, map, stars, ..
        } = self;

        let mut matches = true;

        matches &= criteria.stars.contains(*stars);
        matches &= (score, map).matches(criteria);

        matches
    }
}

impl Searchable for TopEntry {
    #[inline]
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let Self {
            score, map, stars, ..
        } = self;

        let mut matches = true;

        matches &= criteria.stars.contains(*stars);
        matches &= (score, map).matches(criteria);

        matches
    }
}

impl Searchable for (&'_ ScoreSlim, &'_ OsuMap) {
    fn matches(&self, criteria: &FilterCriteria<'_>) -> bool {
        let (score, map) = *self;

        let mut matches = true;

        let mode = match score.mode {
            GameMode::Osu => Mode::Osu,
            GameMode::Taiko => Mode::Taiko,
            GameMode::Catch => Mode::Catch,
            GameMode::Mania => Mode::Mania,
        };

        let attrs = map
            .pp_map
            .attributes()
            .mode(mode)
            .mods(score.mods.bits())
            .converted(score.mode != map.mode())
            .build();

        let clock_rate = attrs.clock_rate as f32;
        let len = map.seconds_drain() as f32 / clock_rate;

        matches &= criteria.ar.contains(attrs.ar as f32);
        matches &= criteria.cs.contains(attrs.cs as f32);
        matches &= criteria.hp.contains(attrs.hp as f32);
        matches &= criteria.od.contains(attrs.od as f32);
        matches &= criteria.length.contains(len);
        matches &= criteria.bpm.contains(map.bpm() * clock_rate);

        let keys = match score.mods.has_key_mod() {
            Some(GameMods::Key1) => 1.0,
            Some(GameMods::Key2) => 2.0,
            Some(GameMods::Key3) => 3.0,
            Some(GameMods::Key4) => 4.0,
            Some(GameMods::Key5) => 5.0,
            Some(GameMods::Key6) => 6.0,
            Some(GameMods::Key7) => 7.0,
            Some(GameMods::Key8) => 8.0,
            Some(GameMods::Key9) => 9.0,
            None => map.cs(),
            _ => unreachable!(),
        };

        matches &= score.mode != GameMode::Mania || criteria.keys.contains(keys);

        if matches && criteria.has_search_terms() {
            let artist = map.artist().cow_to_ascii_lowercase();
            let creator = map.creator().cow_to_ascii_lowercase();
            let title = map.title().cow_to_ascii_lowercase();
            let version = map.version().cow_to_ascii_lowercase();

            matches &= criteria.artist.matches(artist.as_ref());
            matches &= criteria.creator.matches(creator.as_ref());
            matches &= criteria.title.matches(title.as_ref());

            let terms = [artist, creator, title, version];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)));
        }

        matches
    }
}
