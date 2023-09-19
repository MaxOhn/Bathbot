use std::{fs, path::PathBuf};

use skia_safe::font_style::{Slant, Weight};

use crate::error::CardError;

pub(crate) struct FontData {
    normal: WeightData,
    italic: WeightData,
}

pub(crate) struct WeightData {
    w100: Box<[u8]>,
    w300: Box<[u8]>,
    w400: Box<[u8]>,
    w500: Box<[u8]>,
    w700: Box<[u8]>,
    w900: Box<[u8]>,
}

impl FontData {
    pub(crate) fn get(&self, weight: Weight, slant: Slant) -> &[u8] {
        match slant {
            Slant::Upright => self.normal.get(weight),
            Slant::Italic => self.italic.get(weight),
            Slant::Oblique => unimplemented!(),
        }
    }

    pub(crate) fn new(assets: PathBuf) -> Result<Self, CardError> {
        let mut path = FontPath::new(assets);

        let normal = WeightData {
            w100: path.load("Roboto-Thin.ttf")?,
            w300: path.load("Roboto-Light.ttf")?,
            w400: path.load("Roboto-Regular.ttf")?,
            w500: path.load("Roboto-Medium.ttf")?,
            w700: path.load("Roboto-Bold.ttf")?,
            w900: path.load("Roboto-Black.ttf")?,
        };

        let italic = WeightData {
            w100: path.load("Roboto-ThinItalic.ttf")?,
            w300: path.load("Roboto-LightItalic.ttf")?,
            w400: path.load("Roboto-Italic.ttf")?,
            w500: path.load("Roboto-MediumItalic.ttf")?,
            w700: path.load("Roboto-BoldItalic.ttf")?,
            w900: path.load("Roboto-BlackItalic.ttf")?,
        };

        Ok(Self { normal, italic })
    }
}

impl WeightData {
    pub(crate) fn get(&self, weight: Weight) -> &[u8] {
        match *weight {
            100 | 200 => &self.w100,
            300 => &self.w300,
            400 => &self.w400,
            500 => &self.w500,
            600 | 700 => &self.w700,
            800 | 900 => &self.w900,
            other => panic!("invalid weight {other}"),
        }
    }
}

struct FontPath {
    path: PathBuf,
}

impl FontPath {
    fn new(mut assets: PathBuf) -> Self {
        assets.push("fonts");

        Self { path: assets }
    }

    fn load(&mut self, filename: &str) -> Result<Box<[u8]>, CardError> {
        self.path.push(filename);

        let data = fs::read(&self.path).map_err(|source| CardError::LoadFont {
            source,
            path: self.path.display().to_string().into_boxed_str(),
        })?;

        self.path.pop();

        Ok(data.into_boxed_slice())
    }
}
