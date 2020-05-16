mod ranked_score;

pub use self::ranked_score::*;

use serenity::framework::standard::macros::group;

#[group]
#[description = "Commands that can only be used in the belgian osu discord server"]
#[commands(rankedscore)]
struct OsuBelgium;
