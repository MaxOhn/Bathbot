use std::sync::Arc;

use bathbot_model::{PullRequest, Tag};
use bathbot_util::{
    datetime::DATE_FORMAT, numbers::last_multiple, AuthorBuilder, EmbedBuilder, FooterBuilder,
};
use eyre::{Report, Result, WrapErr};
use futures::future::BoxFuture;
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle, SelectMenu, SelectMenuOption},
        embed::EmbedField,
        Component,
    },
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage},
    commands::utility::ChangelogTagPages,
    core::Context,
    util::{interaction::InteractionComponent, Authored, ComponentExt, Emote},
};

pub struct ChangelogPagination {
    tags: Vec<Tag>,
    tag_pages: Vec<ChangelogTagPages>,
    pull_requests: Vec<PullRequest>,
    next_cursor: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: ChangelogPages,
}

impl IActiveMessage for ChangelogPagination {
    fn build_page(&mut self, ctx: Arc<Context>) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page(ctx))
    }

    fn build_components(&self) -> Vec<Component> {
        self.pages.components(&self.tags)
    }

    fn handle_component<'a>(
        &'a mut self,
        ctx: Arc<Context>,
        component: &'a mut InteractionComponent,
    ) -> BoxFuture<'a, ComponentResult> {
        Box::pin(self.async_handle_component(ctx, component))
    }
}

impl ChangelogPagination {
    pub fn new(
        tags: Vec<Tag>,
        tag_pages: Vec<ChangelogTagPages>,
        pull_requests: Vec<PullRequest>,
        next_cursor: Box<str>,
        msg_owner: Id<UserMarker>,
    ) -> Self {
        Self {
            pages: ChangelogPages::new(tags.len()),
            tags,
            tag_pages,
            pull_requests,
            next_cursor,
            msg_owner,
        }
    }

    fn defer(&self) -> bool {
        self.pages.tag_idx >= self.tag_pages.len()
    }

    async fn async_build_page(&mut self, ctx: Arc<Context>) -> Result<BuildPage> {
        let defer = self.defer();

        let pages = loop {
            if self.tag_pages.len() > self.pages.tag_idx {
                let pages = &self.tag_pages[self.pages.tag_idx].pages;
                self.pages.set_amount(pages.len());

                break pages;
            }

            let start_date = self.tags[self.tag_pages.len()].date;
            let end_date = self.tags[self.tag_pages.len() + 1].date;

            let page_fut = ChangelogTagPages::new(
                &ctx,
                &mut self.pull_requests,
                start_date,
                end_date,
                &mut self.next_cursor,
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

        let tag = &self.tags[self.pages.tag_idx];
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

    async fn async_handle_component(
        &mut self,
        ctx: Arc<Context>,
        component: &mut InteractionComponent,
    ) -> ComponentResult {
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
                        if let Err(err) = component.defer(&ctx).await.map_err(Report::new) {
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
            tag_idx: 0,
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
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index() == self.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.index() == self.last_index(),
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
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
            options,
            placeholder: None,
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
