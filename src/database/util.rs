use crate::{bail, bg_game::MapsetTags, BotResult};

pub trait CustomSQL: Sized + std::fmt::Write {
    fn pop(&mut self) -> Option<char>;

    /// Adds (a,b,c,...) to self
    fn in_clause<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: std::fmt::Display,
    {
        let _ = self.write_str(" (");
        for value in values {
            let _ = write!(self, "{},", value);
        }
        self.pop();
        let _ = self.write_char(')');
        self
    }

    /// Adds a delim b delim c delim... without whitespaces to self
    fn set_tags(mut self, delim: &str, tags: MapsetTags, value: bool) -> BotResult<Self> {
        let mut tags = tags.into_iter();
        let first_tag = match tags.next() {
            Some(first_tag) => first_tag,
            None => bail!("cannot build update query without tags"),
        };
        let _ = write!(self, " {}={}", tag_column(first_tag), value as u8);
        for tag in tags {
            let _ = write!(self, "{}{}={}", delim, tag_column(tag), value as u8);
        }
        Ok(self)
    }
}

impl CustomSQL for String {
    fn pop(&mut self) -> Option<char> {
        self.pop()
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
