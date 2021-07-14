use crate::{
    database::{MedalGroup, OsuMedal},
    embeds::{attachment, Author, EmbedFields, Footer},
    util::{constants::OSU_BASE, numbers::round},
};

use hashbrown::HashMap;
use rosu_v2::model::user::User;

pub struct MedalStatsEmbed {
    author: Author,
    fields: EmbedFields,
    footer: Footer,
    image: String,
    thumbnail: String,
}

impl MedalStatsEmbed {
    pub fn new(mut user: User, medals: HashMap<u32, OsuMedal>, with_graph: bool) -> Self {
        let mut fields = Vec::with_capacity(5);

        // Be sure owned medals are sorted by date
        let owned = user.medals.as_mut().unwrap();
        owned.sort_unstable_by_key(|m| m.achieved_at);

        fields.push(field!(
            "Medals",
            format!("{} / {}", owned.len(), medals.len()),
            true
        ));

        let completion = round(100.0 * owned.len() as f32 / medals.len() as f32);
        fields.push(field!("Completion", format!("{}%", completion), true));

        if let Some(medal) = owned.first() {
            let name = medals
                .get(&medal.medal_id)
                .map_or("Unknown medal", |medal| medal.name.as_str());

            let value = format!("{} ({})", name, medal.achieved_at.format("%F"));
            fields.push(field!("First medal", value, false));
        }

        if let Some(medal) = owned.last() {
            let name = medals
                .get(&medal.medal_id)
                .map_or("Unknown medal", |medal| medal.name.as_str());

            let value = format!("{} ({})", name, medal.achieved_at.format("%F"));
            fields.push(field!("Last medal", value, false));
        }

        if !owned.is_empty() {
            let mut counts = HashMap::new();

            // Count groups for all medals
            for medal in medals.values() {
                let (total, _) = counts.entry(medal.grouping).or_insert((0, 0));
                *total += 1;
            }

            // Count groups for owned medals
            for medal in owned.iter() {
                let entry = medals
                    .get(&medal.medal_id)
                    .and_then(|medal| counts.get_mut(&medal.grouping));

                if let Some((_, owned)) = entry {
                    *owned += 1;
                }
            }

            // Add to fields
            let mut add_group_field = |group: MedalGroup| {
                if let Some((total, owned)) = counts.get(&group) {
                    fields.push(field!(
                        group.to_string(),
                        format!("{} / {}", owned, total),
                        true
                    ));
                }
            };

            add_group_field(MedalGroup::Skill);
            add_group_field(MedalGroup::Dedication);
            add_group_field(MedalGroup::HushHush);
            add_group_field(MedalGroup::BeatmapPacks);
            add_group_field(MedalGroup::BeatmapChallengePacks);
            add_group_field(MedalGroup::SeasonalSpotlights);
            add_group_field(MedalGroup::BeatmapSpotlights);
            add_group_field(MedalGroup::ModIntroduction);
        }

        let author = Author::new(user.username)
            .url(format!("{}u/{}", OSU_BASE, user.user_id))
            .icon_url(format!(
                "{}/images/flags/{}.png",
                OSU_BASE, user.country_code
            ));

        let footer = Footer::new("Check osekai.net for more info");

        let image = with_graph
            .then(|| attachment("medal_graph.png"))
            .unwrap_or_default();

        Self {
            image,
            author,
            fields,
            footer,
            thumbnail: user.avatar_url,
        }
    }
}

impl_builder!(MedalStatsEmbed {
    author,
    fields,
    footer,
    image,
    thumbnail,
});
