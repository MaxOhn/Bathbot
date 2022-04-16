use std::fmt;

use twilight_model::{
    application::component::{button::ButtonStyle, ActionRow, Button, Component},
    channel::ReactionType,
};

use crate::util::Emote;

pub use self::{state::GameState, state_info::GameStateInfo};

mod state;
mod state_info;

pub mod components;

#[derive(Copy, Clone)]
pub enum HlVersion {
    ScorePp,
}

#[derive(Copy, Clone, Debug)]
enum HlGuess {
    Higher,
    Lower,
}

impl fmt::Display for HlGuess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

pub struct HigherLowerComponents {
    higher: Button,
    lower: Button,
    next: Button,
    retry: Button,
}

impl HigherLowerComponents {
    pub fn new() -> Self {
        let higher = Button {
            custom_id: Some("higher_button".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Higher".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let lower = Button {
            custom_id: Some("lower_button".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Lower".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let next = Button {
            custom_id: Some("next_higherlower".to_owned()),
            disabled: false,
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: Some("Next".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
        };

        let retry = Button {
            custom_id: Some("try_again_button".to_owned()),
            disabled: false,
            emoji: Some(ReactionType::Unicode {
                name: "🔁".to_owned(),
            }),
            label: Some("Try Again".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
        };

        Self {
            higher,
            lower,
            next,
            retry,
        }
    }

    pub fn disable_higherlower(mut self) -> Self {
        self.higher.disabled = true;
        self.lower.disabled = true;

        self
    }

    pub fn disable_next(mut self) -> Self {
        self.next.disabled = true;

        self
    }

    pub fn disable_restart(mut self) -> Self {
        self.retry.disabled = true;

        self
    }

    pub fn give_up() -> Vec<Component> {
        let give_up_button = Button {
            custom_id: Some("give_up_button".to_owned()),
            disabled: false,
            emoji: None,
            label: Some("Give Up".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let button_row = ActionRow {
            components: vec![Component::Button(give_up_button)],
        };

        vec![Component::ActionRow(button_row)]
    }
}

impl From<HigherLowerComponents> for Vec<Component> {
    fn from(components: HigherLowerComponents) -> Self {
        let HigherLowerComponents {
            higher,
            lower,
            next,
            retry,
        } = components;

        let button_row = ActionRow {
            components: vec![
                Component::Button(higher),
                Component::Button(lower),
                Component::Button(next),
                Component::Button(retry),
            ],
        };

        vec![Component::ActionRow(button_row)]
    }
}
