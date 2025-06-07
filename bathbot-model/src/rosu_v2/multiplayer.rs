use rkyv::{Archive, Serialize, with::Map};
use rosu_mods::{GameMode, GameMods};
use rosu_v2::prelude::{Beatmap, PlaylistItem, Room};
use time::OffsetDateTime;

use crate::rkyv_util::time::DateTimeRkyv;

#[derive(Archive, Serialize)]
#[rkyv(remote = Room, archived = ArchivedRoom)]
pub struct RoomRkyv {
    pub room_id: u64,
    pub name: String,
    #[rkyv(with = DateTimeRkyv)]
    pub starts_at: OffsetDateTime,
    pub participant_count: usize,
    #[rkyv(with = Map<PlaylistItemRkyv>)]
    pub current_playlist_item: Option<PlaylistItem>,
}

#[derive(Archive, Serialize)]
#[rkyv(remote = PlaylistItem, archived = ArchivedPlaylistItem)]
pub struct PlaylistItemRkyv {
    #[rkyv(with = BeatmapRkyv)]
    pub map: Beatmap,
    pub playlist_item_id: u32,
    pub mode: GameMode,
    pub required_mods: GameMods,
}

#[derive(Archive, Serialize)]
#[rkyv(remote = Beatmap, archived = ArchivedBeatmap)]
pub struct BeatmapRkyv {
    pub map_id: u32,
    pub mapset_id: u32,
    pub checksum: Option<String>,
}
