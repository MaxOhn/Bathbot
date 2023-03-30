pub(crate) use self::{
    multi::{MemberSerializer, MultiSerializer},
    single::SingleSerializer,
};

mod multi;
mod single;

const CHANNEL_SCRATCH_SIZE: usize = 16;
const CURRENT_USER_SCRATCH_SIZE: usize = 0;
const GUILD_SCRATCH_SIZE: usize = 0;
const MEMBER_SCRATCH_SIZE: usize = 16;
const ROLE_SCRATCH_SIZE: usize = 0;
const USER_SCRATCH_SIZE: usize = 0;
