use crate::core::{commands::CommandOrigin, Context};
use eyre::Result;

use super::RelaxTop;

pub async fn relax_top(orig: CommandOrigin<'_>, args: RelaxTop<'_>) -> Result<()> {
    todo!()
}

struct RelaxTopArgs {
    name: Option<String>,
}

pub async fn top(orig: CommandOrigin<'_>, args: RelaxTop<'_>) -> Result<()> {
    let msg_owner = orig.user_id()?;
    todo!();
}
