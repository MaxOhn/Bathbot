use std::fmt::Write;

use bathbot_macros::EmbedData;
use bathbot_psql::model::configs::SkinEntry;
use bathbot_util::{constants::OSU_BASE, FooterBuilder};

use crate::pagination::Pages;

#[derive(EmbedData)]
pub struct SkinsEmbed {
    description: String,
    footer: FooterBuilder,
    title: String,
}

// TODO: replace with RankingEmbed once it supports code block hyperlinks
impl SkinsEmbed {
    pub fn new(entries: &[SkinEntry], pages: &Pages) -> Self {
        let idx = pages.index();
        let end_left = entries.len().min(idx + 10);

        let left = &entries[idx..end_left];

        let right = (entries.len() > idx + 10)
            .then(|| {
                let end_right = entries.len().min(idx + 20);

                &entries[idx + 10..end_right]
            })
            .unwrap_or(&[]);

        let left_lengths = Lengths::new(idx, left);
        let right_lengths = Lengths::new(idx + 10, right);

        // Ensuring the right side has ten elements for the zip
        let user_iter = left.iter().zip((0..10).map(|i| right.get(i)));

        let mut description = String::with_capacity(1024);

        for ((left, right), idx) in user_iter.zip(idx + 1..) {
            let _ = write!(
                description,
                "`#{idx:<idx_len$}` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) [`Skin`]({skin_url})",
                idx_len = left_lengths.idx,
                name = left.username,
                name_len = left_lengths.name,
                user_id = left.user_id,
                skin_url = left.skin_url,
            );

            if let Some(right) = right {
                let _ = write!(
                    description,
                    "|`#{idx:<idx_len$}` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) [`Skin`]({skin_url})",
                    idx_len = right_lengths.idx,
                    name = right.username,
                    name_len = right_lengths.name,
                    user_id = right.user_id,
                    skin_url = right.skin_url,
                );
            }

            description.push('\n');
        }

        let title = "All linked skins:".to_owned();

        let page = pages.curr_page();
        let pages = pages.last_page();
        let footer = FooterBuilder::new(format!("Page {page}/{pages}"));

        Self {
            description,
            footer,
            title,
        }
    }
}

struct Lengths {
    idx: usize,
    name: usize,
}

impl Lengths {
    fn new(start: usize, iter: &[SkinEntry]) -> Self {
        let mut idx_len = 0;
        let mut name_len = 0;

        for (entry, i) in iter.iter().zip(start + 1..) {
            let mut idx = i + 1;
            let mut len = 0;

            while idx > 0 {
                len += 1;
                idx /= 10;
            }

            idx_len = idx_len.max(len);
            name_len = name_len.max(entry.username.len());
        }

        Lengths {
            idx: idx_len,
            name: name_len,
        }
    }
}
