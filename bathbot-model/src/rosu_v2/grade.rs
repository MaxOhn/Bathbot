use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Fallible,
};
use rosu_v2::model::Grade;

pub struct GradeRkyv;

#[repr(u8)]
enum ArchivedTag {
    F,
    D,
    C,
    B,
    A,
    S,
    SH,
    X,
    XH,
}

impl From<u8> for ArchivedTag {
    fn from(byte: u8) -> Self {
        match byte {
            0 => ArchivedTag::F,
            1 => ArchivedTag::D,
            2 => ArchivedTag::C,
            3 => ArchivedTag::B,
            4 => ArchivedTag::A,
            5 => ArchivedTag::S,
            6 => ArchivedTag::SH,
            7 => ArchivedTag::X,
            8 => ArchivedTag::XH,
            _ => unreachable!(),
        }
    }
}

impl From<ArchivedTag> for Grade {
    fn from(tag: ArchivedTag) -> Self {
        match tag {
            ArchivedTag::F => Grade::F,
            ArchivedTag::D => Grade::D,
            ArchivedTag::C => Grade::C,
            ArchivedTag::B => Grade::B,
            ArchivedTag::A => Grade::A,
            ArchivedTag::S => Grade::S,
            ArchivedTag::SH => Grade::SH,
            ArchivedTag::X => Grade::X,
            ArchivedTag::XH => Grade::XH,
        }
    }
}

impl From<Grade> for ArchivedTag {
    fn from(grade: Grade) -> Self {
        match grade {
            Grade::F => ArchivedTag::F,
            Grade::D => ArchivedTag::D,
            Grade::C => ArchivedTag::C,
            Grade::B => ArchivedTag::B,
            Grade::A => ArchivedTag::A,
            Grade::S => ArchivedTag::S,
            Grade::SH => ArchivedTag::SH,
            Grade::X => ArchivedTag::X,
            Grade::XH => ArchivedTag::XH,
        }
    }
}

impl ArchiveWith<Grade> for GradeRkyv {
    type Archived = u8;
    type Resolver = ();

    unsafe fn resolve_with(field: &Grade, _: usize, (): Self::Resolver, out: *mut Self::Archived) {
        out.cast::<ArchivedTag>().write(ArchivedTag::from(*field));
    }
}

impl<S: Fallible> SerializeWith<Grade, S> for GradeRkyv {
    fn serialize_with(_: &Grade, _: &mut S) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(())
    }
}

impl<D: Fallible> DeserializeWith<u8, Grade, D> for GradeRkyv {
    fn deserialize_with(field: &u8, _: &mut D) -> Result<Grade, <D as Fallible>::Error> {
        Ok(Grade::from(ArchivedTag::from(*field)))
    }
}
