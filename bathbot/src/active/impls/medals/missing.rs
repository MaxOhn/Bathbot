use std::fmt::{Display, Formatter, Result as FmtResult, Write};

use bathbot_macros::PaginationBuilder;
use bathbot_model::OsekaiMedal;
use bathbot_util::{
    constants::OSU_BASE, osu::flag_url, AuthorBuilder, CowUtils, EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{MedalMissingOrder, MedalType},
    manager::redis::osu::CachedOsuUser,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsMissingPagination {
    user: CachedOsuUser,
    #[pagination(per_page = 15)]
    medals: Box<[MedalType]>,
    medal_count: (usize, usize),
    sort: MedalMissingOrder,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsMissingPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        let pages = &self.pages;
        let idx = pages.index();

        let limit = self.medals.len().min(idx + pages.per_page());
        let includes_last = limit == self.medals.len();
        let medals = &self.medals[idx..limit];

        let mut description = String::new();

        for (i, medal) in medals.iter().enumerate() {
            match medal {
                MedalType::Group(g) => {
                    let _ = writeln!(description, "__**{g}:**__");

                    if let Some(MedalType::Group(_)) = medals.get(i + 1) {
                        description.push_str("All medals acquired\n");
                    } else if i == medals.len() - 1 && includes_last {
                        description.push_str("All medals acquired");
                    }
                }
                MedalType::Medal(m) => {
                    let url = match m.url() {
                        Ok(url) => url,
                        Err(err) => {
                            warn!(?err);

                            m.backup_url()
                        }
                    };

                    let url = url.cow_replace("%25", "%");

                    let _ = writeln!(
                        description,
                        "- [{name}]({url} \"{hover}\")",
                        name = m.name,
                        hover = HoverFormatter::new(self.sort, m),
                    );
                }
            }
        }

        let page = pages.curr_page();
        let pages = pages.last_page();

        let footer = FooterBuilder::new(format!(
            "Page {page}/{pages} | Missing {}/{} medals",
            self.medal_count.0, self.medal_count.1
        ));

        let author = AuthorBuilder::new(self.user.username.as_str())
            .url(format!("{OSU_BASE}u/{}", self.user.user_id))
            .icon_url(flag_url(self.user.country_code.as_str()));

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .thumbnail(self.user.avatar_url.as_str())
            .title("Missing medals");

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(modal, self.msg_owner, false, &mut self.pages)
    }
}

enum HoverFormatter {
    Rarity(f32),
    MedalId(u32),
}

impl HoverFormatter {
    fn new(sort: MedalMissingOrder, medal: &OsekaiMedal) -> Self {
        match sort {
            MedalMissingOrder::MedalId => Self::MedalId(medal.medal_id),
            MedalMissingOrder::Alphabet | MedalMissingOrder::Rarity => Self::Rarity(medal.rarity),
        }
    }
}

impl Display for HoverFormatter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            HoverFormatter::Rarity(rarity) => write!(f, "Rarity: {rarity:.2}%"),
            HoverFormatter::MedalId(medal_id) => write!(f, "Medal ID: {medal_id}"),
        }
    }
}
