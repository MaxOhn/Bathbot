use std::fmt::{Display, Formatter, Result as FmtResult};

use bathbot_psql::model::osu::{DbScoreBeatmap, DbScoreBeatmapset};
use bathbot_util::{numbers::round, CowUtils};

pub use self::{
    map::ScoresMapPagination, server::ScoresServerPagination, user::ScoresUserPagination,
};

mod map;
mod server;
mod user;

pub struct MapFormatter<'s> {
    map: Option<&'s DbScoreBeatmap>,
    mapset: Option<&'s DbScoreBeatmapset>,
}

impl<'s> MapFormatter<'s> {
    pub fn new(map: Option<&'s DbScoreBeatmap>, mapset: Option<&'s DbScoreBeatmapset>) -> Self {
        Self { map, mapset }
    }
}

impl Display for MapFormatter<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match (self.map, self.mapset) {
            (Some(map), Some(mapset)) => write!(
                f,
                "{artist} - {title} [{version}]",
                artist = mapset.artist.cow_escape_markdown(),
                title = mapset.title.cow_escape_markdown(),
                version = map.version.cow_escape_markdown()
            ),
            (Some(map), None) => write!(
                f,
                "<unknown mapset> [{version}]",
                version = map.version.cow_escape_markdown()
            ),
            (None, None) => f.write_str("<unknown map>"),
            (None, Some(_)) => unreachable!(),
        }
    }
}

pub struct StarsFormatter {
    stars: Option<f32>,
}

impl StarsFormatter {
    pub fn new(stars: Option<f32>) -> Self {
        Self { stars }
    }
}

impl Display for StarsFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.stars
            .map(round)
            .map_or(Ok(()), |stars| write!(f, " • {stars}★"))
    }
}

pub struct PpFormatter {
    pp: Option<f32>,
}

impl PpFormatter {
    pub fn new(pp: Option<f32>) -> Self {
        Self { pp }
    }
}

impl Display for PpFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self.pp.map(round) {
            Some(pp) => Display::fmt(&pp, f),
            None => f.write_str("-"),
        }
    }
}
