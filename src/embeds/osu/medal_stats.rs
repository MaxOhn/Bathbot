use crate::{
    custom_client::{OsuMedals, OsuProfile},
    embeds::{Author, EmbedData, Footer},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        numbers::round,
    },
};

use std::collections::HashMap;
use twilight_embed_builder::image_source::ImageSource;

pub struct MedalStatsEmbed {
    thumbnail: ImageSource,
    author: Author,
    fields: Vec<(String, String, bool)>,
    footer: Footer,
    image: Option<ImageSource>,
}

impl MedalStatsEmbed {
    pub fn new(profile: OsuProfile, medals: OsuMedals, with_graph: bool) -> Self {
        let mut fields = Vec::with_capacity(5);
        // Be sure owned medals are sorted by date
        let owned = profile.medals;
        fields.push((
            "Medals".to_owned(),
            format!("{} / {}", owned.len(), medals.len()),
            true,
        ));
        let completion = round(100.0 * owned.len() as f32 / medals.len() as f32);
        fields.push(("Completion".to_owned(), format!("{}%", completion), true));
        if let Some(medal) = owned.first() {
            let name = medals
                .get(&medal.medal_id)
                .map_or("Unknown medal", |medal| medal.name.as_str());
            let value = format!("{} ({})", name, medal.achieved_at.format("%F"));
            fields.push(("First medal".to_owned(), value, false));
        }
        if let Some(medal) = owned.last() {
            let name = medals
                .get(&medal.medal_id)
                .map_or("Unknown medal", |medal| medal.name.as_str());
            let value = format!("{} ({})", name, medal.achieved_at.format("%F"));
            fields.push(("Last medal".to_owned(), value, false));
        }
        if !owned.is_empty() {
            let mut counts = HashMap::new();
            // Count groups for all medals
            for medal in medals.values() {
                let (total, _) = counts.entry(medal.grouping.as_str()).or_insert((0, 0));
                *total += 1;
            }
            // Count groups for owned medals
            for medal in owned.iter() {
                let entry = medals
                    .get(&medal.medal_id)
                    .and_then(|medal| counts.get_mut(medal.grouping.as_str()));
                if let Some((_, owned)) = entry {
                    *owned += 1;
                }
            }
            // Add to fields
            add_group_field("Skill", &counts, &mut fields);
            add_group_field("Dedication", &counts, &mut fields);
            add_group_field("Hush-Hush", &counts, &mut fields);
            add_group_field("Beatmap Packs", &counts, &mut fields);
            add_group_field("Seasonal Spotlights", &counts, &mut fields);
            add_group_field("Beatmap Spotlights", &counts, &mut fields);
            add_group_field("Mod Introduction", &counts, &mut fields);
        }
        let author = Author::new(profile.username)
            .url(format!("{}u/{}", OSU_BASE, profile.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, profile.country_code
            ));
        let footer = Footer::new("Check osekai.net for more info");
        let image = if with_graph {
            Some(ImageSource::attachment("medal_graph.png").unwrap())
        } else {
            None
        };
        Self {
            image,
            author,
            fields,
            footer,
            thumbnail: ImageSource::url(format!("{}{}", AVATAR_URL, profile.user_id)).unwrap(),
        }
    }
}

impl EmbedData for MedalStatsEmbed {
    fn author(&self) -> Option<&Author> {
        Some(&self.author)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn image(&self) -> Option<&ImageSource> {
        self.image.as_ref()
    }
}

fn add_group_field(
    group: &str,
    counts: &HashMap<&str, (usize, usize)>,
    fields: &mut Vec<(String, String, bool)>,
) {
    if let Some((total, owned)) = counts.get(group) {
        fields.push((group.to_owned(), format!("{} / {}", owned, total), true));
    }
}
