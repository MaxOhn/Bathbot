use std::{collections::HashMap, marker::PhantomData, mem};

use rosu_pp::{Beatmap, DifficultyAttributes};
use rosu_v2::model::{score::Score, GameMode};
use skia_safe::{EncodedImageFormat, Surface};

use crate::{builder::card::CardBuilder, error::CardError, skills::Skills};

pub struct UsernameNext;
pub struct RanksNext;
pub struct MedalsNext;
pub struct BytesNext;
pub struct ReadyToDraw;

pub(crate) type Maps<S> = HashMap<u32, (Beatmap, DifficultyAttributes), S>;

pub struct Card<'a, Status> {
    pub(crate) skills: Skills,
    pub(crate) user: User<'a>,
    _phantom: PhantomData<Status>,
}

#[derive(Default)]
pub(crate) struct User<'a> {
    pub(crate) name: &'a str,
    pub(crate) rank_global: u32,
    pub(crate) rank_country: u32,
    pub(crate) medals: u32,
    pub(crate) total_medals: u32,
    pub(crate) pfp: &'a [u8],
    pub(crate) flag: &'a [u8],
}

impl<'a, Status> Card<'a, Status> {
    fn cast<NewStatus>(&mut self) -> &mut Card<'a, NewStatus> {
        // SAFETY: only `_phantom` changes which is a ZST
        unsafe { mem::transmute(self) }
    }
}

impl<'a> Card<'a, UsernameNext> {
    pub fn new<S>(mode: GameMode, scores: &[Score], maps: &Maps<S>) -> Result<Self, CardError> {
        Ok(Self {
            skills: Skills::calculate(mode, scores, maps)?,
            user: Default::default(),
            _phantom: Default::default(),
        })
    }
}

impl<'a> Card<'a, UsernameNext> {
    pub fn username(&mut self, name: &'a str) -> &mut Card<'a, RanksNext> {
        self.user.name = name;

        self.cast()
    }
}

impl<'a> Card<'a, RanksNext> {
    pub fn ranks(&mut self, global_rank: u32, country_rank: u32) -> &mut Card<'a, MedalsNext> {
        self.user.rank_global = global_rank;
        self.user.rank_country = country_rank;

        self.cast()
    }
}

impl<'a> Card<'a, MedalsNext> {
    pub fn medals(&mut self, curr_medals: u32, total_medals: u32) -> &mut Card<'a, BytesNext> {
        self.user.medals = curr_medals;
        self.user.total_medals = total_medals;

        self.cast()
    }
}

impl<'a> Card<'a, BytesNext> {
    pub fn bytes(&mut self, pfp: &'a [u8], flag: &'a [u8]) -> &mut Card<'a, ReadyToDraw> {
        self.user.pfp = pfp;
        self.user.flag = flag;

        self.cast()
    }
}

impl Card<'_, ReadyToDraw> {
    pub fn draw(&self) -> Result<Vec<u8>, CardError> {
        let size = (CardBuilder::W, CardBuilder::H);
        let mut surface = Surface::new_raster_n32_premul(size).ok_or(CardError::CreateSurface)?;

        let title = self.skills.title();

        CardBuilder::new(surface.canvas())
            .background(&title)?
            .header(self.skills.mode(), &self.user, &title)?
            .info(&self.user, &self.skills)?
            .footer()?;

        let png_data = surface
            .image_snapshot()
            .encode_to_data(EncodedImageFormat::PNG)
            .ok_or(CardError::EncodeAsPng)?;

        Ok(png_data.as_bytes().to_vec())
    }
}
