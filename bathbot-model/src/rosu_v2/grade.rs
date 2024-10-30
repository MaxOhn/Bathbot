use rkyv::{Archive, Serialize};
use rosu_v2::model::Grade;

#[derive(Archive, Serialize)]
#[rkyv(remote = Grade, archived = ArchivedGrade, resolver = GradeResolver, derive(Copy, Clone))]
pub enum GradeRkyv {
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

impl From<ArchivedGrade> for Grade {
    fn from(grade: ArchivedGrade) -> Self {
        match grade {
            ArchivedGrade::F => Self::F,
            ArchivedGrade::D => Self::D,
            ArchivedGrade::C => Self::C,
            ArchivedGrade::B => Self::B,
            ArchivedGrade::A => Self::A,
            ArchivedGrade::S => Self::S,
            ArchivedGrade::SH => Self::SH,
            ArchivedGrade::X => Self::X,
            ArchivedGrade::XH => Self::XH,
        }
    }
}
