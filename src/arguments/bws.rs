use super::{parse_dotted, try_link_name, Args};
use crate::Context;

pub struct BwsArgs {
    pub name: Option<String>,
    pub rank_range: Option<RankRange>,
}

impl BwsArgs {
    pub fn new(ctx: &Context, args: Args) -> Self {
        let mut name = None;
        let mut rank_range = None;
        for arg in args {
            match parse_dotted(arg) {
                Some((Some(min), max)) => rank_range = Some(RankRange::Range(min, max)),
                Some((None, rank)) => rank_range = Some(RankRange::Single(rank)),
                None => {
                    if name.is_none() {
                        name = try_link_name(ctx, Some(arg));
                    }
                }
            }
        }
        Self { name, rank_range }
    }
}

pub enum RankRange {
    Single(u32),
    Range(u32, u32),
}
