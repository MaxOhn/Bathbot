use crate::custom_client::OsuStatsMap;

use rosu::model::{ApprovalStatus, Beatmap, GameMode};

pub trait BeatmapExt: Send + Sync {
    fn max_combo(&self) -> Option<u32>;
    fn map_id(&self) -> u32;
    fn mode(&self) -> GameMode;
    fn stars(&self) -> Option<f32>;
    fn approval_status(&self) -> ApprovalStatus;
    fn n_objects(&self) -> Option<u32>;
    fn od(&self) -> f32;
    fn ar(&self) -> f32;
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
    fn n_objects(&self) -> Option<u32> {
        Some(self.count_objects())
    }
    fn od(&self) -> f32 {
        self.diff_od
    }
    fn ar(&self) -> f32 {
        self.diff_ar
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
    fn n_objects(&self) -> Option<u32> {
        None
    }
    fn od(&self) -> f32 {
        self.diff_od
    }
    fn ar(&self) -> f32 {
        self.diff_ar
    }
}
