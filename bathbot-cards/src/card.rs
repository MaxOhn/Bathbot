use std::{collections::HashMap, hash::BuildHasher, marker::PhantomData, mem, path::PathBuf};

use rosu_pp::{Beatmap, DifficultyAttributes};
use rosu_v2::model::{score::Score, GameMode};
use skia_safe::{EncodedImageFormat, Surface};

use crate::{
    builder::card::{CardBuilder, H, W},
    error::CardError,
    font::FontData,
    skills::{CardTitle, Skills},
};

pub struct UserNext;
pub struct RanksNext;
pub struct MedalsNext;
pub struct BytesNext;
pub struct DateNext;
pub struct AssetsPathNext;
pub struct ReadyToDraw;

pub(crate) type Maps<S> = HashMap<u32, (Beatmap, DifficultyAttributes), S>;

pub struct BathbotCard<'a, Status> {
    pub(crate) skills: Skills,
    pub(crate) title: CardTitle,
    pub(crate) inner: CardInner<'a>,
    _phantom: PhantomData<Status>,
}

#[derive(Default)]
pub(crate) struct CardInner<'a> {
    pub(crate) username: &'a str,
    pub(crate) level: f32,
    pub(crate) rank_global: u32,
    pub(crate) rank_country: u32,
    pub(crate) medals: u32,
    pub(crate) total_medals: u32,
    pub(crate) pfp: &'a [u8],
    pub(crate) flag: &'a [u8],
    pub(crate) date: &'a str,
    pub(crate) assets: PathBuf,
}

impl<'a, Status> BathbotCard<'a, Status> {
    fn cast<NewStatus>(&mut self) -> &mut BathbotCard<'a, NewStatus> {
        // SAFETY: only `_phantom` changes which is a ZST
        unsafe { mem::transmute(self) }
    }
}

impl<'a> BathbotCard<'a, UserNext> {
    pub fn new<S>(mode: GameMode, scores: &[Score], maps: Maps<S>) -> Self
    where
        S: BuildHasher,
    {
        let skills = Skills::calculate(mode, scores, maps);

        Self {
            title: CardTitle::new(&skills, scores),
            skills,
            inner: CardInner::default(),
            _phantom: PhantomData,
        }
    }
}

impl<'a> BathbotCard<'a, UserNext> {
    pub fn user(&mut self, name: &'a str, level: f32) -> &mut BathbotCard<'a, RanksNext> {
        self.inner.username = name;
        self.inner.level = level;

        self.cast()
    }
}

impl<'a> BathbotCard<'a, RanksNext> {
    pub fn ranks(
        &mut self,
        global_rank: u32,
        country_rank: u32,
    ) -> &mut BathbotCard<'a, MedalsNext> {
        self.inner.rank_global = global_rank;
        self.inner.rank_country = country_rank;

        self.cast()
    }
}

impl<'a> BathbotCard<'a, MedalsNext> {
    pub fn medals(
        &mut self,
        curr_medals: u32,
        total_medals: u32,
    ) -> &mut BathbotCard<'a, BytesNext> {
        self.inner.medals = curr_medals;
        self.inner.total_medals = total_medals;

        self.cast()
    }
}

impl<'a> BathbotCard<'a, BytesNext> {
    pub fn bytes(&mut self, pfp: &'a [u8], flag: &'a [u8]) -> &mut BathbotCard<'a, DateNext> {
        self.inner.pfp = pfp;
        self.inner.flag = flag;

        self.cast()
    }
}

impl<'a> BathbotCard<'a, DateNext> {
    pub fn date(&mut self, date: &'a str) -> &mut BathbotCard<'a, AssetsPathNext> {
        self.inner.date = date;

        self.cast()
    }
}

impl<'a> BathbotCard<'a, AssetsPathNext> {
    pub fn assets(&mut self, path: PathBuf) -> &mut BathbotCard<'a, ReadyToDraw> {
        self.inner.assets = path;

        self.cast()
    }
}

impl BathbotCard<'_, ReadyToDraw> {
    pub fn draw(&self) -> Result<Vec<u8>, CardError> {
        let fonts = FontData::new(self.inner.assets.clone())?;

        let mut surface = Surface::new_raster_n32_premul((W, H)).ok_or(CardError::CreateSurface)?;

        CardBuilder::new(surface.canvas())
            .draw_background(&self.title, self.inner.assets.clone())?
            .draw_header(self.skills.mode(), &self.inner, &self.title, &fonts)?
            .draw_info(&self.inner, &self.skills, &fonts)?
            .draw_footer(&self.inner, &fonts)?;

        surface
            .image_snapshot()
            .encode_to_data(EncodedImageFormat::PNG)
            .map(|png_data| png_data.as_bytes().to_vec())
            .ok_or(CardError::EncodeAsPng)
    }
}
