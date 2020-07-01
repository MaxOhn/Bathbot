use crate::arguments;

use serenity::framework::standard::Args;
use std::str::FromStr;

pub struct RankArgs {
    pub name: Option<String>,
    pub country: Option<String>,
    pub rank: usize,
}

impl RankArgs {
    pub fn new(mut args: Args) -> Result<Self, String> {
        let mut args = arguments::first_n(&mut args, 2);
        let (country, rank) = if let Some(mut arg) = args.next_back() {
            if let Ok(num) = usize::from_str(&arg) {
                (None, num)
            } else if arg.len() < 3 {
                return Err("Could not parse rank. Provide it either as positive \
                    number or as country acronym followed by a positive \
                    number e.g. `be10`."
                    .to_string());
            } else {
                let num = arg.split_off(2);
                if let Ok(num) = usize::from_str(&num) {
                    (Some(arg.to_uppercase()), num)
                } else {
                    return Err("Could not parse rank. Provide it either as positive \
                                number or as country acronym followed by a positive \
                                number e.g. `be10`."
                        .to_string());
                }
            }
        } else {
            return Err(
                "No rank argument found. Provide it either as positive number or \
                 as country acronym followed by a positive number e.g. `be10`."
                    .to_string(),
            );
        };
        if rank > 10_000 {
            return Err(
                "Unfortunately I can only provide data for ranks up to 10,000 :(".to_string(),
            );
        } else if rank == 0 {
            return Err("Rank must be greater than 0 you clown :^)".to_string());
        }
        Ok(Self {
            name: args.next(),
            country,
            rank,
        })
    }
}
