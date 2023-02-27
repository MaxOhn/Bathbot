pub(crate) use self::{multi::MultiSerializer, single::SingleSerializer};

mod multi;
mod single;

const CHANNEL_SCRATCH_SIZE: usize = 2;
const CURRENT_USER_SCRATCH_SIZE: usize = 0;
const GUILD_SCRATCH_SIZE: usize = 0;
const MEMBER_SCRATCH_SIZE: usize = 0;
const ROLE_SCRATCH_SIZE: usize = 0;
const USER_SCRATCH_SIZE: usize = 0;
