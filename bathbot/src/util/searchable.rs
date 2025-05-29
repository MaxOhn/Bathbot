use std::{borrow::Cow, mem};

use bathbot_model::ScoreSlim;
use bathbot_util::{
    CowUtils,
    query::{FilterCriteria, IFilterCriteria, Operator, RegularCriteria, Searchable},
};
use rosu_pp::{Beatmap, model::beatmap::BeatmapAttributesBuilder};
use rosu_v2::{
    model::GameMode,
    prelude::{GameModIntermode, GameMods, Score},
};

use crate::manager::OsuMap;

/// Native wrapper around [`RegularCriteria`] so that we can implement foreign
/// traits on foreign types
#[derive(Default)]
#[repr(transparent)]
pub struct NativeCriteria<'a>(RegularCriteria<'a>);

impl<'a> NativeCriteria<'a> {
    pub const fn cast<'b>(
        criteria: &'b FilterCriteria<RegularCriteria<'a>>,
    ) -> &'b FilterCriteria<Self> {
        // SAFETY: `NativeCriteria` is a transparent wrapper around
        // `RegularCriteria`
        unsafe { mem::transmute(criteria) }
    }
}

impl<'q> IFilterCriteria<'q> for NativeCriteria<'q> {
    #[inline]
    fn try_parse_key_value(
        &mut self,
        key: Cow<'q, str>,
        value: Cow<'q, str>,
        op: Operator,
    ) -> bool {
        self.0.try_parse_key_value(key, value, op)
    }

    #[inline]
    fn any_field(&self) -> bool {
        self.0.any_field()
    }

    #[inline]
    fn display(&self, content: &mut String) {
        self.0.display(content);
    }
}

impl Searchable<NativeCriteria<'_>> for Beatmap {
    fn matches(&self, criteria: &FilterCriteria<NativeCriteria<'_>>) -> bool {
        let mut matches = true;

        matches &= criteria.0.ar.contains(self.ar);
        matches &= criteria.0.cs.contains(self.cs);
        matches &= criteria.0.hp.contains(self.hp);
        matches &= criteria.0.od.contains(self.od);

        matches
    }
}

impl Searchable<NativeCriteria<'_>> for Score {
    fn matches(&self, criteria: &FilterCriteria<NativeCriteria<'_>>) -> bool {
        let mut matches = true;

        let mut artist = Cow::default();
        let mut creator = Cow::default();
        let mut title = Cow::default();
        let mut version = Cow::default();

        if let Some(ref map) = self.map {
            let attrs = BeatmapAttributesBuilder::default()
                .ar(map.ar, false)
                .cs(map.cs, false)
                .hp(map.hp, false)
                .od(map.od, false)
                .mods(self.mods.clone())
                .mode((map.mode as u8).into(), map.convert)
                .build();

            let clock_rate = attrs.clock_rate as f32;
            let len = map.seconds_drain as f32 / clock_rate;

            matches &= criteria.0.stars.contains(map.stars);
            matches &= criteria.0.ar.contains(attrs.ar as f32);
            matches &= criteria.0.cs.contains(attrs.cs as f32);
            matches &= criteria.0.hp.contains(attrs.hp as f32);
            matches &= criteria.0.od.contains(attrs.od as f32);
            matches &= criteria.0.length.contains(len);
            matches &= criteria.0.bpm.contains(map.bpm * clock_rate);

            let keys = keys(&self.mods, map.cs);
            matches &= map.mode != GameMode::Mania || criteria.0.keys.contains(keys);

            version = map.version.cow_to_ascii_lowercase();
        }

        if let Some(mapset) = self.mapset.as_ref().filter(|_| matches) {
            artist = mapset.artist.cow_to_ascii_lowercase();
            creator = mapset.creator_name.cow_to_ascii_lowercase();
            title = mapset.title.cow_to_ascii_lowercase();

            matches &= criteria.0.artist.matches(artist.as_ref());
            matches &= criteria.0.creator.matches(creator.as_ref());
            matches &= criteria.0.title.matches(title.as_ref());
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

impl Searchable<NativeCriteria<'_>> for (&'_ ScoreSlim, &'_ OsuMap) {
    fn matches(&self, criteria: &FilterCriteria<NativeCriteria<'_>>) -> bool {
        let (score, map) = *self;

        let mut matches = true;

        let attrs = map.attributes().mods(score.mods.clone()).build();

        let clock_rate = attrs.clock_rate as f32;
        let len = map.seconds_drain() as f32 / clock_rate;

        matches &= criteria.0.ar.contains(attrs.ar as f32);
        matches &= criteria.0.cs.contains(attrs.cs as f32);
        matches &= criteria.0.hp.contains(attrs.hp as f32);
        matches &= criteria.0.od.contains(attrs.od as f32);
        matches &= criteria.0.length.contains(len);
        matches &= criteria.0.bpm.contains(map.bpm() * clock_rate);

        matches &= score.mode != GameMode::Mania
            || criteria.0.keys.contains(keys(&score.mods, attrs.cs as f32));

        if matches && criteria.has_search_terms() {
            let artist = map.artist().cow_to_ascii_lowercase();
            let creator = map.creator().cow_to_ascii_lowercase();
            let title = map.title().cow_to_ascii_lowercase();
            let version = map.version().cow_to_ascii_lowercase();

            matches &= criteria.0.artist.matches(artist.as_ref());
            matches &= criteria.0.creator.matches(creator.as_ref());
            matches &= criteria.0.title.matches(title.as_ref());

            let terms = [artist, creator, title, version];

            matches &= criteria
                .search_terms()
                .all(|term| terms.iter().any(|searchable| searchable.contains(term)));
        }

        matches
    }
}

fn keys(mods: &GameMods, cs: f32) -> f32 {
    [
        (GameModIntermode::OneKey, 1.0),
        (GameModIntermode::TwoKeys, 2.0),
        (GameModIntermode::ThreeKeys, 3.0),
        (GameModIntermode::FourKeys, 4.0),
        (GameModIntermode::FiveKeys, 5.0),
        (GameModIntermode::SixKeys, 6.0),
        (GameModIntermode::SevenKeys, 7.0),
        (GameModIntermode::EightKeys, 8.0),
        (GameModIntermode::NineKeys, 9.0),
        (GameModIntermode::TenKeys, 10.0),
    ]
    .into_iter()
    .find_map(|(gamemod, keys)| mods.contains_intermode(gamemod).then_some(keys))
    .unwrap_or(cs)
}
