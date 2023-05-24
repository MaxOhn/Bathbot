use std::{
    fmt::{Display, Formatter, Result as FmtResult, Write},
    sync::Arc,
};

use bathbot_macros::PaginationBuilder;
use bathbot_model::{rosu_v2::user::User, OsekaiMedal};
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
    core::Context,
    manager::redis::RedisData,
    util::interaction::{InteractionComponent, InteractionModal},
};

#[derive(PaginationBuilder)]
pub struct MedalsMissingPagination {
    user: RedisData<User>,
    #[pagination(per_page = 15)]
    medals: Box<[MedalType]>,
    medal_count: (usize, usize),
    sort: MedalMissingOrder,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MedalsMissingPagination {
    fn build_page(&mut self, _: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
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
                    let _ = writeln!(
                        description,
                        "- [{name}](https://osekai.net/medals/?medal={url_name} \"{hover}\")",
                        name = m.name,
                        url_name = m.name.cow_replace(' ', "+"),
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

        let (country_code, username, user_id, avatar_url) = match self.user {
            RedisData::Original(ref user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_ref();

                (country_code, username, user_id, avatar_url)
            }
            RedisData::Archive(ref user) => {
                let country_code = user.country_code.as_str();
                let username = user.username.as_str();
                let user_id = user.user_id;
                let avatar_url = user.avatar_url.as_ref();

                (country_code, username, user_id, avatar_url)
            }
        };

        let author = AuthorBuilder::new(username)
            .url(format!("{OSU_BASE}u/{user_id}"))
            .icon_url(flag_url(country_code));

        let embed = EmbedBuilder::new()
            .author(author)
            .description(description)
            .footer(footer)
            .thumbnail(avatar_url)
            .title("Missing medals");

        BuildPage::new(embed, false).boxed()
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components()
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: &'a Context,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        handle_pagination_component(ctx, component, self.msg_owner, false, &mut self.pages)
    }

    fn handle_modal<'a>(
        &'a mut self,
        ctx: &'a Context,
        modal: &'a mut InteractionModal,
    ) -> BoxFuture<'a, Result<()>> {
        handle_pagination_modal(ctx, modal, self.msg_owner, false, &mut self.pages)
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
