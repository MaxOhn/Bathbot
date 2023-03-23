use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

use twilight_model::channel::message::{
    component::{ActionRow, Button, ButtonStyle, Component},
    ReactionType,
};

use crate::util::Emote;

pub use self::state::GameState;

mod farm_map;
mod kind;
mod score_pp;
mod state;

pub mod components;
pub mod retry;

const W: u32 = 900;
const H: u32 = 250;

#[derive(Copy, Clone, Debug)]
enum HlGuess {
    Higher,
    Lower,
}

impl Display for HlGuess {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self, f)
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

fn mapset_cover(mapset_id: u32) -> String {
    format!("https://assets.ppy.sh/beatmaps/{mapset_id}/covers/cover.jpg")
}
