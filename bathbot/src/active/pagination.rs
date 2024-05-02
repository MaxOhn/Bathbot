use bathbot_util::{
    modal::{ModalBuilder, TextInputBuilder},
    numbers::last_multiple,
};
use eyre::{ContextCompat, Result, WrapErr};
use futures::{future::BoxFuture, FutureExt};
use twilight_model::{
    channel::message::{
        component::{ActionRow, Button, ButtonStyle},
        Component,
    },
    id::{marker::UserMarker, Id},
};

use super::ComponentResult;
use crate::util::{
    interaction::{InteractionComponent, InteractionModal},
    Authored, ComponentExt, Emote, ModalExt,
};

#[derive(Clone, Debug)]
pub struct Pages {
    index: usize,
    last_index: usize,
    per_page: usize,
}

impl Pages {
    /// `per_page`: How many entries per page
    ///
    /// `amount`: How many entries in total
    pub fn new(per_page: usize, amount: usize) -> Self {
        Self {
            index: 0,
            per_page,
            last_index: last_multiple(per_page, amount),
        }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn last_index(&self) -> usize {
        self.last_index
    }

    pub fn per_page(&self) -> usize {
        self.per_page
    }

    pub fn curr_page(&self) -> usize {
        self.index / self.per_page + 1
    }

    pub fn last_page(&self) -> usize {
        self.last_index / self.per_page + 1
    }

    /// Set and validate the current index
    pub fn set_index(&mut self, new_index: usize) {
        self.index = self.last_index.min(new_index);
    }

    /// Returns pagination components based on the current [`Pages`]
    pub fn components(&self) -> Vec<Component> {
        if self.last_index == 0 {
            return Vec::new();
        }

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::JumpStart.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: self.index == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_custom = Button {
            custom_id: Some("pagination_custom".to_owned()),
            disabled: false,
            emoji: Some(Emote::MyPosition.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: self.index == self.last_index,
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.index == self.last_index,
            emoji: Some(Emote::JumpEnd.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
        };

        let components = vec![
            Component::Button(jump_start),
            Component::Button(single_step_back),
            Component::Button(jump_custom),
            Component::Button(single_step),
            Component::Button(jump_end),
        ];

        vec![Component::ActionRow(ActionRow { components })]
    }
}

pub fn handle_pagination_component<'a>(
    component: &'a mut InteractionComponent,
    msg_owner: Id<UserMarker>,
    defer: bool,
    pages: &'a mut Pages,
) -> BoxFuture<'a, ComponentResult> {
    let fut = async move {
        async_handle_pagination_component(component, msg_owner, defer, pages)
            .await
            .unwrap_or_else(ComponentResult::Err)
    };

    fut.boxed()
}

async fn async_handle_pagination_component(
    component: &mut InteractionComponent,
    msg_owner: Id<UserMarker>,
    defer: bool,
    pages: &mut Pages,
) -> Result<ComponentResult> {
    if component.user_id()? != msg_owner {
        return Ok(ComponentResult::Ignore);
    }

    match component.data.custom_id.as_str() {
        "pagination_start" => {
            if defer {
                component
                    .defer()
                    .await
                    .wrap_err("Failed to defer component")?;
            }

            pages.set_index(0);
        }
        "pagination_back" => {
            if defer {
                component
                    .defer()
                    .await
                    .wrap_err("Failed to defer component")?;
            }

            pages.set_index(pages.index().saturating_sub(pages.per_page()));
        }
        "pagination_step" => {
            if defer {
                component
                    .defer()
                    .await
                    .wrap_err("Failed to defer component")?;
            }

            pages.set_index(pages.index() + pages.per_page());
        }
        "pagination_end" => {
            if defer {
                component
                    .defer()
                    .await
                    .wrap_err("Failed to defer component")?;
            }

            pages.set_index(pages.last_index());
        }
        "pagination_custom" => {
            let max_page = pages.last_page();
            let placeholder = format!("Number between 1 and {max_page}");

            let input = TextInputBuilder::new("page_input", "Page number")
                .min_len(1)
                .max_len(5)
                .placeholder(placeholder);

            let modal = ModalBuilder::new("pagination_page", "Jump to a page").input(input);

            return Ok(ComponentResult::CreateModal(modal));
        }
        other => {
            warn!(name = %other, ?component, "Unknown pagination component");

            return Ok(ComponentResult::Ignore);
        }
    }

    Ok(ComponentResult::BuildPage)
}

pub fn handle_pagination_modal<'a>(
    modal: &'a mut InteractionModal,
    msg_owner: Id<UserMarker>,
    defer: bool,
    pages: &'a mut Pages,
) -> BoxFuture<'a, Result<()>> {
    Box::pin(async_handle_pagination_modal(
        modal, msg_owner, defer, pages,
    ))
}

async fn async_handle_pagination_modal(
    modal: &mut InteractionModal,
    msg_owner: Id<UserMarker>,
    defer: bool,
    pages: &mut Pages,
) -> Result<()> {
    if modal.user_id()? != msg_owner {
        return Ok(());
    }

    let input = modal
        .data
        .components
        .first()
        .and_then(|row| row.components.first())
        .wrap_err("Missing modal input")?;

    let Some(Ok(page)) = input.value.as_deref().map(str::parse) else {
        debug!(input = input.value, "Failed to parse page input as usize");

        return Ok(());
    };

    let max_page = pages.last_page();

    if !(1..=max_page).contains(&page) {
        debug!("Page {page} is not between 1 and {max_page}");

        return Ok(());
    }

    if defer {
        modal.defer().await.wrap_err("Failed to defer modal")?;
    }

    pages.set_index((page - 1) * pages.per_page());

    Ok(())
}
