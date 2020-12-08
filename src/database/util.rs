use crate::{bail, bg_game::MapsetTags, BotResult};

use std::fmt::{Display, Write};

pub trait CustomSQL: Sized + Write {
    fn pop(&mut self) -> Option<char>;

    fn in_clause<I, T>(self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Display;

    /// Adds a delim b delim c delim... without whitespaces to self
    fn set_tags(self, delim: &str, tags: MapsetTags, value: bool) -> BotResult<Self>;
}

impl CustomSQL for String {
    fn pop(&mut self) -> Option<char> {
        self.pop()
    }

    /// Adds (a,b,c,...) to self
    fn in_clause<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Display,
    {
        let iter = values.into_iter();
        match iter.size_hint() {
            (0, _) => return self,
            (len, _) => self.reserve(3 + len * 8),
        }
        let _ = self.write_str(" (");
        for value in iter {
            let _ = write!(self, "{},", value);
        }
        self.pop();
        let _ = self.write_char(')');
        self
    }

    /// Adds a delim b delim c delim... without whitespaces to self
    fn set_tags(mut self, delim: &str, tags: MapsetTags, value: bool) -> BotResult<Self> {
        let size = tags.size();
        let mut tags = tags.into_iter();
        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => bail!("cannot build update query without tags"),
        };
        self.reserve(size * (delim.len() + 10));
        write!(self, " {}={}", tag_column(first_tag), value)?;
        for tag in tags {
            write!(self, "{}{}={}", delim, tag_column(tag), value)?;
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
