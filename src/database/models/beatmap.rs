use std::convert::TryInto;

use rosu_v2::model::beatmap::*;
use sqlx::FromRow;
use time::OffsetDateTime;

#[derive(Debug, FromRow)]
#[sqlx(type_name = "maps")]
pub struct DBBeatmap {
    pub map_id: i32,
    pub mapset_id: i32,
    pub checksum: Option<String>,
    pub version: String,
    pub seconds_total: i32,
    pub seconds_drain: i32,
    pub count_circles: i32,
    pub count_sliders: i32,
    pub count_spinners: i32,
    pub hp: f32,
    pub cs: f32,
    pub od: f32,
    pub ar: f32,
    pub mode: i16,
    pub status: i16,
    pub last_update: OffsetDateTime,
    pub stars: f32,
    pub bpm: f32,
    pub max_combo: Option<i32>,
    pub user_id: i32,
}

impl From<DBBeatmap> for BeatmapCompact {
    fn from(map: DBBeatmap) -> Self {
        BeatmapCompact {
            checksum: map.checksum,
            creator_id: map.user_id as u32,
            fail_times: None,
            map_id: map.map_id as u32,
            mapset: None,
            max_combo: map.max_combo.map(|n| n as u32),
            mode: (map.mode as u8).into(),
            seconds_total: map.seconds_total as u32,
            stars: map.stars,
            status: (map.status as i8).try_into().unwrap(),
            version: map.version,
        }
    }
}

impl From<DBBeatmap> for Beatmap {
    fn from(map: DBBeatmap) -> Self {
        Beatmap {
            ar: map.ar,
            bpm: map.bpm,
            checksum: map.checksum,
            convert: false,
            count_circles: map.count_circles as u32,
            count_sliders: map.count_sliders as u32,
            count_spinners: map.count_spinners as u32,
            creator_id: map.user_id as u32,
            cs: map.cs,
            deleted_at: None,
            fail_times: None,
            hp: map.hp,
            is_scoreable: true,
            last_updated: map.last_update,
            map_id: map.map_id as u32,
            mapset: None,
            mapset_id: map.mapset_id as u32,
            max_combo: map.max_combo.map(|n| n as u32),
            mode: (map.mode as u8).into(),
            od: map.od,
            passcount: 0,
            playcount: 0,
            seconds_drain: map.seconds_drain as u32,
            seconds_total: map.seconds_total as u32,
            stars: map.stars,
            status: (map.status as i8).try_into().unwrap(),
            url: format!("https://osu.ppy.sh/beatmaps/{}", map.map_id),
            version: map.version,
        }
    }
}

#[derive(Debug, FromRow)]
#[sqlx(type_name = "mapsets")]
pub struct DBBeatmapset {
    pub mapset_id: i32,
    pub user_id: i32,
    pub artist: String,
    pub title: String,
    pub creator: String,
    pub status: i16,
    pub ranked_date: OffsetDateTime,
    pub bpm: f32,
}

impl From<DBBeatmapset> for BeatmapsetCompact {
    fn from(mapset: DBBeatmapset) -> Self {
        BeatmapsetCompact {
            artist: mapset.artist,
            artist_unicode: None,
            covers: BeatmapsetCovers {
                cover: String::new(),
                cover_2x: String::new(),
                card: String::new(),
                card_2x: String::new(),
                list: String::new(),
                list_2x: String::new(),
                slim_cover: String::new(),
                slim_cover_2x: String::new(),
            },
            creator_name: mapset.creator.into(),
            creator_id: mapset.user_id as u32,
            favourite_count: 0,
            genre: None,
            hype: None,
            language: None,
            mapset_id: mapset.mapset_id as u32,
            nsfw: false,
            playcount: 0,
            preview_url: format!("b.ppy.sh/preview/{}.mp3", mapset.mapset_id),
            source: String::new(),
            status: (mapset.status as i8).try_into().unwrap(),
            title: mapset.title,
            title_unicode: None,
            video: false,
        }
    }
}

impl From<DBBeatmapset> for Beatmapset {
    fn from(mapset: DBBeatmapset) -> Self {
        Beatmapset {
            artist: mapset.artist,
            artist_unicode: None,
            availability: BeatmapsetAvailability {
                download_disabled: false,
                more_information: None,
            },
            bpm: mapset.bpm,
            can_be_hyped: false,
            converts: None,
            covers: BeatmapsetCovers {
                cover: String::new(),
                cover_2x: String::new(),
                card: String::new(),
                card_2x: String::new(),
                list: String::new(),
                list_2x: String::new(),
                slim_cover: String::new(),
                slim_cover_2x: String::new(),
            },
            creator: None,
            creator_name: mapset.creator.into(),
            creator_id: mapset.user_id as u32,
            description: None,
            discussion_enabled: false,
            discussion_locked: true,
            favourite_count: 0,
            genre: None,
            hype: None,
            is_scoreable: true,
            language: None,
            last_updated: mapset.ranked_date,
            legacy_thread_url: None,
            maps: None,
            mapset_id: mapset.mapset_id as u32,
            nominations_summary: BeatmapsetNominations {
                current: 0,
                required: 0,
            },
            nsfw: false,
            playcount: 0,
            preview_url: format!("b.ppy.sh/preview/{}.mp3", mapset.mapset_id),
            ratings: None,
            ranked_date: Some(mapset.ranked_date),
            recent_favourites: None,
            source: String::new(),
            status: (mapset.status as i8).try_into().unwrap(),
            storyboard: false,
            submitted_date: None,
            tags: String::new(),
            title: mapset.title,
            title_unicode: None,
            video: false,
        }
    }
}
