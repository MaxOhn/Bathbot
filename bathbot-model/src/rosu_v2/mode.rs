use rkyv::{niche::niching::Niching, Archive, Place};
use rosu_mods::GameMode;

pub struct GameModeNiche;

impl GameModeNiche {
    const NICHED: u8 = u8::MAX;
}

impl Niching<GameMode> for GameModeNiche {
    unsafe fn is_niched(niched: *const GameMode) -> bool {
        unsafe { *niched as u8 == Self::NICHED }
    }

    fn resolve_niched(out: Place<GameMode>) {
        Self::NICHED.resolve((), unsafe { out.cast_unchecked() });
    }
}
