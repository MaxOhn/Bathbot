use crate::{
    commands::osu::CommonUser,
    embeds::{EmbedData, Footer},
    util::constants::OSU_BASE,
};

use rosu_v2::model::score::Score;
use smallvec::SmallVec;
use std::fmt::Write;
use twilight_embed_builder::image_source::ImageSource;

pub struct CommonEmbed {
    description: String,
    thumbnail: ImageSource,
    footer: Footer,
}

type CommonScore = SmallVec<[(usize, f32, Score); 3]>;

impl CommonEmbed {
    pub fn new(users: &[CommonUser], scores: &[CommonScore], index: usize) -> Self {
        let mut description = String::with_capacity(512);

        for (i, scores) in scores.iter().enumerate() {
            let (title, version, map_id) = {
                let (_, _, first) = scores.first().unwrap();
                let map = first.map.as_ref().unwrap();

                (
                    &first.mapset.as_ref().unwrap().title,
                    &map.version,
                    map.map_id,
                )
            };

            let _ = writeln!(
                description,
                "**{idx}.** [{title} [{version}]]({base}b/{id})",
                idx = index + i + 1,
                title = title,
                version = version,
                base = OSU_BASE,
                id = map_id,
            );

            description.push('-');

            for (pos, pp, score) in scores.iter() {
                let _ = write!(
                    description,
                    " :{medal}_place: `{name}`: {pp:.2}pp",
                    medal = match pos {
                        0 => "first",
                        1 => "second",
                        2 => "third",
                        _ => unreachable!(),
                    },
                    name = score.user.as_ref().unwrap().username,
                    pp = pp,
                );
            }

            description.push('\n');
        }

        description.pop();

        let mut footer = String::with_capacity(64);
        footer.push_str("🥇 count");

        for user in users {
            let _ = write!(footer, " | {}: {}", user.name(), user.first_count);
        }

        Self {
            footer: Footer::new(footer),
            description,
            thumbnail: ImageSource::attachment("avatar_fuse.png").unwrap(),
        }
    }
}

impl EmbedData for CommonEmbed {
    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
