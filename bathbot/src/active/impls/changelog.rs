use bathbot_model::{PullRequestsAndTags, Tag};
use bathbot_util::{
    AuthorBuilder, Authored, EmbedBuilder, FooterBuilder, datetime::DATE_FORMAT,
    numbers::last_multiple,
};
use eyre::{Report, Result, WrapErr};
use twilight_model::{
    channel::message::{
        Component,
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption, SelectMenuType},
        embed::EmbedField,
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::utility::ChangelogTagPages,
    util::{ComponentExt, Emote, interaction::InteractionComponent},
};

pub struct ChangelogPagination {
    tag_pages: Vec<ChangelogTagPages>,
    data: PullRequestsAndTags,
    msg_owner: Id<UserMarker>,
    pages: ChangelogPages,
}

impl IActiveMessage for ChangelogPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let defer = self.defer();

        let pages = loop {
            if self.tag_pages.len() > self.pages.tag_idx {
                let pages = &self.tag_pages[self.pages.tag_idx].pages;
                self.pages.set_amount(pages.len());

                break pages;
            }

            let page_fut = ChangelogTagPages::new(
                &mut self.data,
                self.tag_pages.len(),
                self.tag_pages.len() + 1,
            );

            let pages = page_fut.await.wrap_err("Failed to build tag pages")?;
            self.tag_pages.push(pages);
        };

        fn add_field(name: &str, lines: &[Box<str>], fields: &mut Vec<EmbedField>) {
            if lines.is_empty() {
                return;
            }

            let len = lines.iter().fold(0, |len, line| len + line.len() + 1);
            let mut value = String::with_capacity(len);

            for line in lines {
                value.push_str(line);
                value.push('\n');
            }

            let field = EmbedField {
                inline: false,
                name: name.to_owned(),
                value,
            };

            fields.push(field);
        }

        let fields = if pages.is_empty() {
            None
        } else {
            let page = &pages[self.pages.index];

            let mut fields = Vec::with_capacity(
                !page.features.is_empty() as usize
                    + !page.fixes.is_empty() as usize
                    + !page.adjustments.is_empty() as usize
                    + !page.other.is_empty() as usize,
            );

            add_field("Features", &page.features, &mut fields);
            add_field("Fixes", &page.fixes, &mut fields);
            add_field("Adjustments", &page.adjustments, &mut fields);
            add_field("Other", &page.other, &mut fields);

            Some(fields)
        };

        let author =
            AuthorBuilder::new("Bathbot changelog").url("https://github.com/MaxOhn/Bathbot");

        let curr_page = self.pages.curr_page();
        let last_page = self.pages.last_page();
        let footer = FooterBuilder::new(format!("Page {curr_page}/{last_page}",));

        let tag = &self.data.tags[self.pages.tag_idx];
        let title = format!("{} ({})", tag.name, tag.date.format(DATE_FORMAT).unwrap());

        let mut embed = EmbedBuilder::new()
            .author(author)
            .footer(footer)
            .title(title);

        embed = if let Some(fields) = fields {
            embed.fields(fields)
        } else {
            embed.description("No PRs found")
        };

        Ok(BuildPage::new(embed, defer))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components(&self.data.tags)
    }

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        let user_id = match component.user_id() {
            Ok(user_id) => user_id,
            Err(err) => return ComponentResult::Err(err),
        };

        if user_id != self.msg_owner {
            return ComponentResult::Ignore;
        }

        match component.data.custom_id.as_str() {
            "pagination_start" => self.pages.set_index(0),
            "pagination_back" => {
                let new_index = self.pages.index().saturating_sub(self.pages.per_page());
                self.pages.set_index(new_index);
            }
            "pagination_step" => {
                let new_index = self.pages.index() + self.pages.per_page();
                self.pages.set_index(new_index);
            }
            "pagination_end" => self.pages.set_index(self.pages.last_index()),
            "changelog_menu" => {
                let Some(name) = component.data.values.pop() else {
                    return ComponentResult::Err(eyre!("Missing value in changelog menu"));
                };

                let Some(idx) = self
                    .data
                    .tags
                    .iter()
                    .position(|tag| tag.name.as_ref() == name.as_str())
                else {
                    return ComponentResult::Err(eyre!("Missing tag name `{name}`"));
                };

                if self.pages.tag_idx != idx {
                    self.pages.tag_idx = idx;
                    self.pages.set_index(0);

                    if self.defer() {
                        if let Err(err) = component.defer().await.map_err(Report::new) {
                            return ComponentResult::Err(err.wrap_err("Failed to defer component"));
                        }
                    }
                }
            }
            other => {
                warn!(name = %other, ?component, "Unknown changelog component");

                return ComponentResult::Ignore;
            }
        }

        ComponentResult::BuildPage
    }
}

impl ChangelogPagination {
    pub fn new(
        tag_pages: Vec<ChangelogTagPages>,
        data: PullRequestsAndTags,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            pages: ChangelogPages::new(data.tags.len()),
            tag_pages,
            data,
            msg_owner,
        }
    }

    fn defer(&self) -> bool {
        self.pages.tag_idx >= self.tag_pages.len()
    }
}

struct ChangelogPages {
    index: usize,
    last_index: usize,
    tag_idx: usize,
}

impl ChangelogPages {
    const PER_PAGE: usize = 1;

    pub fn new(amount: usize) -> Self {
        Self {
            index: 0,
            last_index: last_multiple(Self::PER_PAGE, amount),
            tag_idx: 1, // initially skip the spoofed "upcoming" tag
        }
    }

    const fn index(&self) -> usize {
        self.index
    }

    const fn last_index(&self) -> usize {
        self.last_index
    }

    const fn per_page(&self) -> usize {
        Self::PER_PAGE
    }

    const fn curr_page(&self) -> usize {
        self.index() / self.per_page() + 1
    }

    fn last_page(&self) -> usize {
        self.last_index() / self.per_page() + 1
    }

    fn set_index(&mut self, new_index: usize) {
        self.index = self.last_index().min(new_index);
    }

    fn set_amount(&mut self, amount: usize) {
        self.last_index = last_multiple(Self::PER_PAGE, amount);
    }

    fn components(&self, tags: &[Tag]) -> Vec<Component> {
        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.index() == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index() == self.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.index() == self.last_index(),
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let buttons = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(single_step),
            Component::Button(jump_end),
        ];

        let options = tags
            .iter()
            .take(25)
            .enumerate()
            .map(|(i, tag)| SelectMenuOption {
                default: i == self.tag_idx,
                description: None,
                emoji: None,
                label: String::from(tag.name.clone()),
                value: String::from(tag.name.clone()),
            })
            .collect();

        let menu = SelectMenu {
            custom_id: "changelog_menu".to_string(),
            disabled: false,
            max_values: None,
            min_values: None,
            options: Some(options),
            placeholder: None,
            channel_types: None,
            default_values: None,
            kind: SelectMenuType::Text,
        };

        let menu = vec![Component::SelectMenu(menu)];

        vec![
            Component::ActionRow(ActionRow {
                components: buttons,
            }),
            Component::ActionRow(ActionRow { components: menu }),
        ]
    }
}
