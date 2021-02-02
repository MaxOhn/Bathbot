use crate::{
    commands::osu::Difference,
    custom_client::SnipeRecent,
    embeds::{osu, Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        datetime::how_long_ago,
        error::PPError,
        numbers::round,
        osu::prepare_beatmap_file,
    },
    BotResult,
};

use rosu::model::User;
use rosu_pp::{Beatmap, BeatmapExt};
use std::{fmt::Write, fs::File};
use twilight_embed_builder::image_source::ImageSource;

pub struct SnipedDiffEmbed {
    description: String,
    thumbnail: ImageSource,
    title: &'static str,
    author: Author,
    footer: Footer,
}

impl SnipedDiffEmbed {
    pub async fn new<'i, S>(
        user: &User,
        diff: Difference,
        scores: S,
        total: usize,
        pages: (usize, usize),
    ) -> BotResult<Self>
    where
        S: Iterator<Item = &'i SnipeRecent>,
    {
        let idx = (pages.0 - 1) * 5 + 1;
        let mut description = String::with_capacity(512);

        for (i, score) in scores.enumerate() {
            let stars = match score.stars {
                Some(stars) => stars,
                None => {
                    let map_path = prepare_beatmap_file(score.beatmap_id).await?;
                    let file = File::open(map_path).map_err(PPError::from)?;
                    let map = Beatmap::parse(file).map_err(PPError::from)?;

                    map.stars(score.mods.bits(), None).stars()
                }
            };

            let _ = write!(
                description,
                "**{idx}. [{map}]({base}b/{id}) {mods}** [{stars}]\n{acc} ~ ",
                idx = idx + i,
                map = score.map,
                base = OSU_BASE,
                id = score.beatmap_id,
                mods = osu::get_mods(score.mods),
                stars = osu::get_stars(stars),
                acc = score.accuracy,
            );

            let _ = match diff {
                Difference::Gain => match score.sniped {
                    Some(ref name) => write!(
                        description,
                        "Sniped [{name}]({base}u/{id}) ",
                        name = name,
                        base = OSU_BASE,
                        id = score.sniped_id.unwrap_or(2),
                    ),
                    None => write!(description, "Unclaimed until "),
                },
                Difference::Loss => write!(
                    description,
                    "Sniped by [{name}]({base}u/{id}) ",
                    name = score.sniper,
                    base = OSU_BASE,
                    id = score.sniper_id,
                ),
            };

            description += &how_long_ago(&score.date);
            description.push('\n');
        }

        description.pop();

        let title = match diff {
            Difference::Gain => "New national #1s since last week",
            Difference::Loss => "Lost national #1s since last week",
        };

        Ok(Self {
            title,
            description,
            author: osu::get_user_author(user),
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, user.user_id)).unwrap(),
            footer: Footer::new(format!("Page {}/{} ~ Total: {}", pages.0, pages.1, total)),
        })
    }
}

impl EmbedData for SnipedDiffEmbed {
    fn title(&self) -> Option<&str> {
        Some(self.title)
    }

    fn description(&self) -> Option<&str> {
        Some(&self.description)
    }

    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }

    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }

    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
}
