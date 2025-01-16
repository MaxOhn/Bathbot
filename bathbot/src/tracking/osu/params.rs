use std::fmt::{Display, Formatter, Result as FmtResult};

use bathbot_psql::model::osu::DbTrackedOsuUserInChannel;
use rosu_v2::model::GameMode;

#[derive(Copy, Clone)]
pub struct TrackEntryParams {
    /// `1..=100`
    index: Range<u8>,
    /// `0.0..`
    pp: Range<f32>,
    /// 0.0..=100.0
    combo_percent: Range<f32>,
}

impl TrackEntryParams {
    pub const DEFAULT_MAX_COMBO_PERCENT: f32 = 100.0;
    pub const DEFAULT_MAX_INDEX: u8 = 100;
    pub const DEFAULT_MAX_PP: f32 = f32::INFINITY;
    pub const DEFAULT_MIN_COMBO_PERCENT: f32 = 0.0;
    pub const DEFAULT_MIN_INDEX: u8 = 1;
    pub const DEFAULT_MIN_PP: f32 = 0.0;
    // Compile-time assertion that `TrackEntryParams` is no larger than 20
    // bytes so that its `Copy` impl is justified.
    const _SMALL_ENOUGH: [(); 0] = [(); 0 - { 1 - (size_of::<TrackEntryParams>() <= 20) as usize }];

    pub const fn new() -> Self {
        Self {
            index: Range::new_raw(Self::DEFAULT_MIN_INDEX, Self::DEFAULT_MAX_INDEX),
            pp: Range::new_raw(Self::DEFAULT_MIN_PP, Self::DEFAULT_MAX_PP),
            combo_percent: Range::new_raw(
                Self::DEFAULT_MIN_COMBO_PERCENT,
                Self::DEFAULT_MAX_COMBO_PERCENT,
            ),
        }
    }

    pub fn with_index(self, min: Option<u8>, max: Option<u8>) -> Self {
        Self {
            index: Range::<u8>::new(min, max, Self::DEFAULT_MIN_INDEX, Self::DEFAULT_MAX_INDEX),
            ..self
        }
    }

    pub fn with_pp(self, min: Option<f32>, max: Option<f32>) -> Self {
        Self {
            pp: Range::<f32>::new(min, max, Self::DEFAULT_MIN_PP, Self::DEFAULT_MAX_PP),
            ..self
        }
    }

    pub fn with_combo_percent(self, min: Option<f32>, max: Option<f32>) -> Self {
        Self {
            combo_percent: Range::<f32>::new(
                min,
                max,
                Self::DEFAULT_MIN_COMBO_PERCENT,
                Self::DEFAULT_MAX_COMBO_PERCENT,
            ),
            ..self
        }
    }

    pub const fn index(&self) -> Range<u8> {
        self.index
    }

    pub const fn pp(&self) -> Range<f32> {
        self.pp
    }

    pub const fn combo_percent(&self) -> Range<f32> {
        self.combo_percent
    }

    pub const fn matches(&self, idx: u8, pp: f32, combo_percent: Option<f32>) -> bool {
        self.index.contains(idx)
            || self.pp.contains(pp)
            || match combo_percent {
                // Manual `Option::is_some_and` to preserve const-ness
                Some(percent) => self.combo_percent.contains(percent),
                None => false,
            }
    }

    pub(super) const fn into_db_entry(
        self,
        user_id: u32,
        mode: GameMode,
    ) -> DbTrackedOsuUserInChannel {
        DbTrackedOsuUserInChannel {
            user_id: user_id as i32,
            gamemode: mode as i16,
            min_index: Some(self.index.start as i16),
            max_index: Some(self.index.end as i16),
            min_pp: Some(self.pp.start),
            max_pp: Some(self.pp.end),
            min_combo_percent: Some(self.combo_percent.start),
            max_combo_percent: Some(self.combo_percent.end),
        }
    }
}

impl From<DbTrackedOsuUserInChannel> for TrackEntryParams {
    fn from(entry: DbTrackedOsuUserInChannel) -> Self {
        const fn map_as_u8(opt: Option<i16>) -> Option<u8> {
            match opt {
                Some(val) => Some(val as u8),
                None => None,
            }
        }

        Self::new()
            .with_index(map_as_u8(entry.min_index), map_as_u8(entry.max_index))
            .with_pp(entry.min_pp, entry.max_pp)
            .with_combo_percent(entry.min_combo_percent, entry.max_combo_percent)
    }
}

impl Default for TrackEntryParams {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! const_helpers {
    ( $ty:ty, $unwrap_or:ident, $clamp:ident ) => {
        impl Range<$ty> {
            fn new(start: Option<$ty>, end: Option<$ty>, min: $ty, max: $ty) -> Self {
                let start = start.unwrap_or(min).clamp(min, max);
                let end = end.unwrap_or(max).clamp(min, max);

                Self::new_raw(start, end.max(start))
            }

            const fn contains(&self, value: $ty) -> bool {
                value >= self.start && value <= self.end
            }
        }
    };
}

const_helpers!(u8, unwrap_or_u8, clamp_u8);
const_helpers!(f32, unwrap_or_f32, clamp_f32);

/// Custom [`std::ops::RangeInclusive`].
#[derive(Copy, Clone)]
pub struct Range<T> {
    start: T,
    end: T,
}

impl<T> Range<T> {
    // `start` must not be greater than `end`
    const fn new_raw(start: T, end: T) -> Self {
        Self { start, end }
    }
}

impl<T: Display> Display for Range<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}-{}", self.start, self.end)
    }
}
