use std::fmt::{Display, Formatter, Result as FmtResult};

use rosu_v2::{
    model::{GameMode, mods::GameModIntermode, score::Score},
    mods,
};

enum TitleDescription {
    NoMod,
    DoubleTime,
    HalfTime,
    HardRockOsu,
    HiddenOsu,
    Traceable,
    Flashlight,
    SpinOut,
    EasyOsu,
    FewNoMod,
    Versatile,
    HardRockTaiko,
    HardRockCatch,
    EasyCatch,
    HiddenCatch,
    Hacking,
    HiddenMania,
    Mirror,
    EasyMania,
    Lazer,
    Key(usize),
    MultiKey,
}

impl Display for TitleDescription {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let desc = match self {
            Self::NoMod => "Mod-Hating",
            Self::DoubleTime => "Speedy",
            Self::HalfTime => "Slow-Mo",
            Self::HardRockOsu => "Ant-Clicking",
            Self::HiddenOsu => "HD-Abusing",
            Self::Traceable => "Hitcircle-Hating",
            Self::Flashlight => "Blindsighted",
            Self::SpinOut => "Lazy-Spin",
            Self::EasyOsu => "Patient",
            Self::FewNoMod => "Mod-Loving",
            Self::Versatile => "Versatile",
            Self::HardRockTaiko => "Zooming",
            Self::HardRockCatch => "Pea-Catching",
            Self::EasyCatch => "Training-Wheels",
            Self::HiddenCatch => "Ghost-Fruit",
            Self::Hacking => "Hacking",
            Self::HiddenMania => "Brain-Lag",
            Self::Mirror => "Unmindblockable",
            Self::EasyMania => "3-Life",
            Self::Lazer => "New-Skool",
            Self::Key(key) => return write!(f, "{key}K"),
            Self::MultiKey => "Multi-Key",
        };

        f.write_str(desc)
    }
}

#[derive(Default)]
pub(crate) struct TitleDescriptions(Vec<TitleDescription>);

impl TitleDescriptions {
    const CL_COUNT: usize = 50;
    const DT_COUNT: usize = 60;
    const EZ_COUNT: usize = 15;
    const FL_COUNT: usize = 15;
    const HD_COUNT: usize = 60;
    const HR_COUNT: usize = 60;
    const HT_COUNT: usize = 30;
    const KEY_COUNT: usize = 70;
    const MR_COUNT: usize = 30;
    const NM_COUNT: usize = 70;
    const NO_NM_COUNT: usize = 10;
    const SO_COUNT: usize = 20;
    const TC_COUNT: usize = 60;

    pub(crate) fn new(mode: GameMode, scores: &[Score], legacy_scores: bool) -> Self {
        let mut nomod = 0;
        let mut hidden = 0;
        let mut doubletime = 0;
        let mut halftime = 0;
        let mut hardrock = 0;
        let mut traceable = 0;
        let mut easy = 0;
        let mut flashlight = 0;
        let mut mirror = 0;
        let mut spunout = 0;
        let mut classic = 0;

        let mut key_counts = [0_u8; 11];

        let dtnc = mods!(DT NC);

        for score in scores {
            let idx = [
                (GameModIntermode::OneKey, 1),
                (GameModIntermode::TwoKeys, 2),
                (GameModIntermode::ThreeKeys, 3),
                (GameModIntermode::FourKeys, 4),
                (GameModIntermode::FiveKeys, 5),
                (GameModIntermode::SixKeys, 6),
                (GameModIntermode::SevenKeys, 7),
                (GameModIntermode::EightKeys, 8),
                (GameModIntermode::NineKeys, 9),
                (GameModIntermode::TenKeys, 10),
            ]
            .into_iter()
            .find_map(|(gamemod, keys)| score.mods.contains_intermode(gamemod).then_some(keys))
            .unwrap_or_else(|| score.map.as_ref().unwrap().cs.round() as usize);

            key_counts[idx] += 1;

            if score.mods.contains_intermode(GameModIntermode::Classic) {
                classic += 1;

                if score.mods.len() == 1 {
                    nomod += 1;
                    continue;
                }
            } else if score.mods.is_empty() {
                nomod += 1;
                continue;
            }

            hidden += score.mods.contains_intermode(GameModIntermode::Hidden) as usize;
            doubletime += score.mods.contains_any(dtnc.clone()) as usize;
            halftime += score.mods.contains_intermode(GameModIntermode::HalfTime) as usize;
            hardrock += score.mods.contains_intermode(GameModIntermode::HardRock) as usize;
            traceable += score.mods.contains_intermode(GameModIntermode::Traceable) as usize;
            easy += score.mods.contains_intermode(GameModIntermode::Easy) as usize;
            flashlight += score.mods.contains_intermode(GameModIntermode::Flashlight) as usize;
            spunout += score.mods.contains_intermode(GameModIntermode::SpunOut) as usize;
            mirror += score.mods.contains_intermode(GameModIntermode::Mirror) as usize;
        }

        let mut mods = Self::default();

        if nomod > Self::NM_COUNT {
            mods.push(TitleDescription::NoMod);
        }

        if classic <= Self::CL_COUNT && !legacy_scores {
            mods.push(TitleDescription::Lazer);
        }

        if doubletime > Self::DT_COUNT {
            mods.push(TitleDescription::DoubleTime);
        }

        if halftime > Self::HT_COUNT {
            mods.push(TitleDescription::HalfTime);
        }

        if flashlight > Self::FL_COUNT {
            mods.push(TitleDescription::Flashlight);
        }

        if spunout > Self::SO_COUNT {
            mods.push(TitleDescription::SpinOut);
        }

        if hardrock > Self::HR_COUNT {
            let desc = match mode {
                GameMode::Osu => TitleDescription::HardRockOsu,
                GameMode::Taiko => TitleDescription::HardRockTaiko,
                GameMode::Catch => TitleDescription::HardRockCatch,
                GameMode::Mania => TitleDescription::Hacking, // HR is unranked in mania
            };

            mods.push(desc);
        }

        if easy > Self::EZ_COUNT {
            let desc = match mode {
                GameMode::Osu | GameMode::Taiko => TitleDescription::EasyOsu,
                GameMode::Catch => TitleDescription::EasyCatch,
                GameMode::Mania => TitleDescription::EasyMania,
            };

            mods.push(desc);
        }

        if hidden > Self::HD_COUNT {
            let desc = match mode {
                GameMode::Osu | GameMode::Taiko => TitleDescription::HiddenOsu,
                GameMode::Catch => TitleDescription::HiddenCatch,
                GameMode::Mania => TitleDescription::HiddenMania,
            };

            mods.push(desc);
        }

        if traceable > Self::TC_COUNT {
            mods.push(TitleDescription::Traceable);
        }

        if mirror > Self::MR_COUNT {
            mods.push(TitleDescription::Mirror);
        }

        if mode == GameMode::Mania {
            let (max_key_idx, max_key) = key_counts
                .into_iter()
                .enumerate()
                .max_by_key(|(_, next)| *next)
                .unwrap_or((0, 0));

            if max_key as usize > Self::KEY_COUNT {
                mods.push(TitleDescription::Key(max_key_idx));
            } else {
                mods.push(TitleDescription::MultiKey);
            }
        }

        if let [] | [TitleDescription::Lazer] = mods.as_slice() {
            if nomod < Self::NO_NM_COUNT {
                TitleDescription::FewNoMod.into()
            } else {
                TitleDescription::Versatile.into()
            }
        } else {
            mods
        }
    }

    fn push(&mut self, desc: TitleDescription) {
        self.0.push(desc);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn as_slice(&self) -> &[TitleDescription] {
        self.0.as_slice()
    }
}

impl From<TitleDescription> for TitleDescriptions {
    #[inline]
    fn from(desc: TitleDescription) -> Self {
        Self(vec![desc])
    }
}

impl Display for TitleDescriptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let mut iter = self.0.iter();

        if let Some(desc) = iter.next() {
            write!(f, "{desc}")?;

            for desc in iter {
                write!(f, " {desc}")?;
            }
        }

        Ok(())
    }
}
