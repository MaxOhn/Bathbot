use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Fallible,
};
use rosu_v2::prelude::GameMode;

pub struct GameModeRkyv;

impl ArchiveWith<GameMode> for GameModeRkyv {
    type Archived = GameMode;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(mode: &GameMode, _: usize, _: Self::Resolver, out: *mut Self::Archived) {
        out.write(*mode)
    }
}

impl<S: Fallible + ?Sized> SerializeWith<GameMode, S> for GameModeRkyv {
    #[inline]
    fn serialize_with(_: &GameMode, _: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(())
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<GameMode, GameMode, D> for GameModeRkyv {
    #[inline]
    fn deserialize_with(mode: &GameMode, _: &mut D) -> Result<GameMode, <D as Fallible>::Error> {
        Ok(*mode)
    }
}
