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
pub mod retry;

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

pub struct HlComponents;

impl HlComponents {
    /// All buttons are disabled
    pub fn disabled() -> Vec<Component> {
        Self::build(Self::buttons())
    }

    /// Only the Higher and Lower buttons are enabled
    pub fn higherlower() -> Vec<Component> {
        let mut buttons = Self::buttons();
        buttons[0].disabled = false;
        buttons[1].disabled = false;

        Self::build(buttons)
    }

    /// Only the Next button is enabled
    pub fn next() -> Vec<Component> {
        let mut buttons = Self::buttons();
        buttons[2].disabled = false;

        Self::build(buttons)
    }

    /// Only the restart button is enabled
    pub fn restart() -> Vec<Component> {
        let mut buttons = Self::buttons();
        buttons[3].disabled = false;

        Self::build(buttons)
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

    fn buttons() -> [Button; 4] {
        let higher = Button {
            custom_id: Some("higher_button".to_owned()),
            disabled: true,
            emoji: None,
            label: Some("Higher".to_owned()),
            style: ButtonStyle::Success,
            url: None,
        };

        let lower = Button {
            custom_id: Some("lower_button".to_owned()),
            disabled: true,
            emoji: None,
            label: Some("Lower".to_owned()),
            style: ButtonStyle::Danger,
            url: None,
        };

        let next = Button {
            custom_id: Some("next_higherlower".to_owned()),
            disabled: true,
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: Some("Next".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
        };

        let retry = Button {
            custom_id: Some("try_again_button".to_owned()),
            disabled: true,
            emoji: Some(ReactionType::Unicode {
                name: "🔁".to_owned(),
            }),
            label: Some("Try Again".to_owned()),
            style: ButtonStyle::Secondary,
            url: None,
        };

        [higher, lower, next, retry]
    }

    fn build(buttons: [Button; 4]) -> Vec<Component> {
        let button_row = ActionRow {
            components: buttons.map(Component::Button).to_vec(),
        };

        vec![Component::ActionRow(button_row)]
    }
}
