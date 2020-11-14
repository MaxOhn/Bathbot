use crate::{
    custom_client::{OsuMedals, OsuProfile},
    embeds::{Author, EmbedData},
    util::{
        constants::{AVATAR_URL, OSU_BASE},
        numbers::round,
    },
};

use std::{cmp::Ordering::Equal, collections::HashMap};
use twilight_embed_builder::image_source::ImageSource;

pub struct MedalStatsEmbed {
    thumbnail: ImageSource,
    author: Author,
    fields: Vec<(String, String, bool)>,
}

impl MedalStatsEmbed {
    pub fn new(profile: OsuProfile, medals: OsuMedals) -> Self {
        let mut fields = Vec::with_capacity(5);
        let mut owned = profile.medals;
        fields.push((
            "Medals".to_owned(),
            format!("{} / {}", owned.len(), medals.len()),
            true,
        ));
        let completion = round(100.0 * owned.len() as f32 / medals.len() as f32);
        fields.push(("Completion".to_owned(), format!("{}%", completion), true));
        owned.sort_by_key(|medal| medal.achieved_at);
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
            for medal in owned {
                let entry = medals
                    .get(&medal.medal_id)
                    .and_then(|medal| counts.get_mut(medal.grouping.as_str()));
                if let Some((_, owned)) = entry {
                    *owned += 1;
                }
            }
            let mut group_counts: Vec<_> = counts.drain().collect();
            // Sort by % completed
            group_counts.sort_unstable_by(|(_, (a_total, a_owned)), (_, (b_total, b_owned))| {
                (*b_owned as f32 / *b_total as f32)
                    .partial_cmp(&(*a_owned as f32 / *a_total as f32))
                    .unwrap_or(Equal)
            });
            // Add to fields
            for (group, (total, owned)) in group_counts {
                fields.push((group.to_owned(), format!("{} / {}", owned, total), true));
            }
        }
        let author = Author::new(profile.username)
            .url(format!("{}u/{}", OSU_BASE, profile.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, profile.country_code
            ));
        Self {
            author,
            fields,
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
}
