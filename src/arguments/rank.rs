use super::Args;

use std::str::FromStr;

pub struct RankArgs {
    pub name: Option<String>,
    pub country: Option<String>,
    pub rank: usize,
}

impl RankArgs {
    pub fn new(args: Args) -> Result<Self, &'static str> {
        let mut args = args.take_all();
        let (country, rank) = if let Some(arg) = args.next_back() {
            if let Ok(num) = usize::from_str(arg) {
                (None, num)
            } else if arg.len() < 3 {
                return Err("Could not parse rank. Provide it either as positive \
                    number or as country acronym followed by a positive \
                    number e.g. `be10`.");
            } else {
                let (country, num) = arg.split_at(2);
                if let Ok(num) = usize::from_str(num) {
                    (Some(country.to_uppercase()), num)
                } else {
                    return Err("Could not parse rank. Provide it either as positive \
                                number or as country acronym followed by a positive \
                                number e.g. `be10`.");
                }
            }
        } else {
            return Err(
                "No rank argument found. Provide it either as positive number or \
                 as country acronym followed by a positive number e.g. `be10`.",
            );
        };
        Ok(Self {
            name: args.next().map(|arg| arg.to_owned()),
            country,
            rank,
        })
    }
}
