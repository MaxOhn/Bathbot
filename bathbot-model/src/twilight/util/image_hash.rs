use std::fmt::{Display, Formatter, Result as FmtResult};

use rkyv::{Archive, Deserialize, Portable, Serialize, bytecheck::CheckBytes};
use twilight_model::util::ImageHash;

#[derive(Copy, Clone, Archive, Serialize, Deserialize, Portable, CheckBytes)]
#[rkyv(remote = ImageHash, as = Self)]
#[bytecheck(crate = rkyv::bytecheck)]
#[repr(C)]
pub struct ImageHashRkyv {
    #[rkyv(getter = get_animated)]
    pub animated: bool,
    #[rkyv(getter = get_bytes)]
    pub bytes: [u8; 16],
}

fn get_animated(image_hash: &ImageHash) -> bool {
    image_hash.is_animated()
}

fn get_bytes(image_hash: &ImageHash) -> [u8; 16] {
    image_hash.bytes()
}

impl ImageHashRkyv {
    pub fn is_eq_opt(this: Option<&Self>, other: Option<&ImageHash>) -> bool {
        match (this, other) {
            (Some(l), Some(r)) => l == r,
            (Some(_), None) => false,
            (None, Some(_)) => false,
            (None, None) => true,
        }
    }
}

impl From<ImageHashRkyv> for ImageHash {
    fn from(image_hash: ImageHashRkyv) -> Self {
        Self::new(image_hash.bytes, image_hash.animated)
    }
}

impl Display for ImageHashRkyv {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        ImageHash::new(self.bytes, self.animated).fmt(f)
    }
}

impl PartialEq<ImageHash> for ImageHashRkyv {
    fn eq(&self, other: &ImageHash) -> bool {
        self.bytes == other.bytes() && self.animated == other.is_animated()
    }
}

impl PartialEq<ImageHashRkyv> for ImageHash {
    fn eq(&self, other: &ImageHashRkyv) -> bool {
        other.eq(self)
    }
}
