use std::fmt::{Display, Formatter, Result as FmtResult};

pub(crate) enum TitleSuffix {
    AllRounder,
    Sniper,
    Ninja,
    RhythmEnjoyer,
    Gunslinger,
    WhackAMole,
    Masher,
    Gamer,
    DropletDodger,
}

impl TitleSuffix {
    const THRESHOLD: f64 = 0.91;

    pub(super) fn osu(acc: f64, aim: f64, speed: f64, max: f64) -> Self {
        let acc = Self::is_within_threshold(acc, max);
        let aim = Self::is_within_threshold(aim, max);
        let speed = Self::is_within_threshold(speed, max);

        match (acc, aim, speed) {
            (true, true, true) => Self::AllRounder,
            (true, true, false) => Self::Sniper,
            (true, false, true) => Self::Ninja,
            (true, false, false) => Self::RhythmEnjoyer,
            (false, true, true) => Self::Gunslinger,
            (false, true, false) => Self::WhackAMole,
            (false, false, true) => Self::Masher,
            (false, false, false) => unreachable!(),
        }
    }

    pub(super) fn taiko(acc: f64, strain: f64, max: f64) -> Self {
        let acc = Self::is_within_threshold(acc, max);
        let strain = Self::is_within_threshold(strain, max);

        match (acc, strain) {
            (true, true) => Self::Gamer,
            (true, false) => Self::RhythmEnjoyer,
            (false, true) => Self::Masher,
            (false, false) => unreachable!(),
        }
    }

    pub(super) fn catch(acc: f64, movement: f64, max: f64) -> Self {
        let acc = Self::is_within_threshold(acc, max);
        let movement = Self::is_within_threshold(movement, max);

        match (acc, movement) {
            (true, true) => Self::Gamer,
            (true, false) => Self::RhythmEnjoyer,
            (false, true) => Self::DropletDodger,
            (false, false) => unreachable!(),
        }
    }

    pub(super) fn mania(acc: f64, strain: f64, max: f64) -> Self {
        let acc = Self::is_within_threshold(acc, max);
        let strain = Self::is_within_threshold(strain, max);

        match (acc, strain) {
            (true, true) => Self::Gamer,
            (true, false) => Self::RhythmEnjoyer,
            (false, true) => Self::Masher,
            (false, false) => unreachable!(),
        }
    }

    fn is_within_threshold(val: f64, max: f64) -> bool {
        val / max > Self::THRESHOLD
    }
}

impl Display for TitleSuffix {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let suffix = match self {
            Self::AllRounder => "All-Rounder",
            Self::Sniper => "Sniper",
            Self::Ninja => "Ninja",
            Self::RhythmEnjoyer => "Rhythm Enjoyer",
            Self::Gunslinger => "Gunslinger",
            Self::WhackAMole => "Whack-A-Mole",
            Self::Masher => "Masher",
            Self::Gamer => "Gamer",
            Self::DropletDodger => "Droplet Dodger",
        };

        f.write_str(suffix)
    }
}
