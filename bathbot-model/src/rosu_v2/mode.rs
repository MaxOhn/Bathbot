use rkyv::{Place, niche::niching::Niching};
use rosu_mods::GameMode;

pub struct GameModeNiche;

impl GameModeNiche {
    const NICHED: u8 = u8::MAX;
}

impl Niching<GameMode> for GameModeNiche {
    unsafe fn is_niched(niched: *const GameMode) -> bool {
        unsafe { *niched.cast::<u8>() == Self::NICHED }
    }

    fn resolve_niched(out: Place<GameMode>) {
        unsafe { out.cast_unchecked::<u8>() }.write(Self::NICHED);
    }
}
