use std::fmt::{Display, Formatter, Result as FmtResult};

use rkyv::{with::ArchiveWith, Archive, Deserialize, Serialize};
use rkyv_with::ArchiveWith;
use twilight_model::util::ImageHash as TwImageHash;

#[derive(Archive, ArchiveWith, Copy, Clone, Deserialize, Serialize)]
#[archive(as = "Self")]
#[archive_with(from(TwImageHash))]
pub struct ImageHash {
    #[archive_with(getter = "TwImageHash::is_animated", getter_owned)]
    pub animated: bool,
    #[archive_with(getter = "TwImageHash::bytes", getter_owned)]
    pub bytes: [u8; 16],
}

impl Display for ImageHash {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(&TwImageHash::new(self.bytes, self.animated), f)
    }
}
