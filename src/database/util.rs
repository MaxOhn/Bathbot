use crate::{bg_game::MapsetTags, BotResult};

use std::fmt::Write;

pub trait CustomSQL: Sized + Write {
    /// Adds a delim b delim c delim... without whitespaces to self
    fn set_tags(self, delim: &str, tags: MapsetTags, value: bool) -> BotResult<Self>;
}

impl CustomSQL for String {
    fn set_tags(mut self, delim: &str, tags: MapsetTags, value: bool) -> BotResult<Self> {
        let size = tags.size();
        let mut tags = tags.into_iter();

        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => bail!("cannot build update query without tags"),
        };

        self.reserve(size * (delim.len() + 10));
        write!(self, " {}={value}", tag_column(first_tag))?;

        for tag in tags {
            write!(self, "{delim}{}={value}", tag_column(tag))?;
        }

        Ok(self)
    }
}

fn tag_column(tag: MapsetTags) -> &'static str {
    match tag {
        MapsetTags::Farm => "farm",
        MapsetTags::Streams => "streams",
        MapsetTags::Alternate => "alternate",
        MapsetTags::BlueSky => "bluesky",
        MapsetTags::Meme => "meme",
        MapsetTags::Old => "old",
        MapsetTags::Easy => "easy",
        MapsetTags::Hard => "hard",
        MapsetTags::Kpop => "kpop",
        MapsetTags::English => "english",
        MapsetTags::HardName => "hardname",
        MapsetTags::Weeb => "weeb",
        MapsetTags::Tech => "tech",
        _ => panic!("Only call tag_column with single tag argument"),
    }
}
