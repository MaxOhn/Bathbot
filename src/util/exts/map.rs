use crate::custom_client::OsuStatsMap;

use rosu::models::{ApprovalStatus, Beatmap, GameMode};

pub trait BeatmapExt {
    fn max_combo(&self) -> Option<u32>;
    fn map_id(&self) -> u32;
    fn mode(&self) -> GameMode;
    fn stars(&self) -> Option<f32>;
    fn approval_status(&self) -> ApprovalStatus;
}

impl BeatmapExt for &Beatmap {
    fn max_combo(&self) -> Option<u32> {
        self.max_combo
    }
    fn map_id(&self) -> u32 {
        self.beatmap_id
    }
    fn mode(&self) -> GameMode {
        self.mode
    }
    fn stars(&self) -> Option<f32> {
        Some(self.stars)
    }
    fn approval_status(&self) -> ApprovalStatus {
        self.approval_status
    }
}

impl BeatmapExt for &OsuStatsMap {
    fn max_combo(&self) -> Option<u32> {
        self.max_combo
    }
    fn map_id(&self) -> u32 {
        self.beatmap_id
    }
    fn mode(&self) -> GameMode {
        self.mode
    }
    fn stars(&self) -> Option<f32> {
        self.stars
    }
    fn approval_status(&self) -> ApprovalStatus {
        self.approval_status
    }
}
