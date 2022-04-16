use std::cmp::Ordering;

use eyre::Report;
use rand::Rng;
use rosu_v2::prelude::GameMode;
use twilight_model::application::component::{button::ButtonStyle, ActionRow, Button, Component};

use crate::{core::Context, BotResult};

pub use self::{state::GameState, state_info::GameStateInfo};

mod state;
mod state_info;

pub mod components;

enum HlGuess {
    Higher,
    Lower,
}

pub fn hl_components() -> Vec<Component> {
    let higher_button = Button {
        custom_id: Some("higher_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Higher".to_owned()),
        style: ButtonStyle::Success,
        url: None,
    };

    let lower_button = Button {
        custom_id: Some("lower_button".to_owned()),
        disabled: false,
        emoji: None,
        label: Some("Lower".to_owned()),
        style: ButtonStyle::Danger,
        url: None,
    };

    let button_row = ActionRow {
        components: vec![
            Component::Button(higher_button),
            Component::Button(lower_button),
        ],
    };

    vec![Component::ActionRow(button_row)]
}

pub async fn random_play(ctx: &Context, prev_pp: f32, curr_score: u32) -> BotResult<GameStateInfo> {
    let max_play = 25 - curr_score.min(24);
    let min_play = 24 - 2 * curr_score.min(12);

    let (rank, play): (u32, u32) = {
        let mut rng = rand::thread_rng();

        (rng.gen_range(1..=5000), rng.gen_range(min_play..max_play))
    };

    let page = ((rank - 1) / 50) + 1;
    let idx = (rank - 1) % 50;

    let player = ctx
        .osu()
        .performance_rankings(GameMode::STD)
        .page(page)
        .await?
        .ranking
        .swap_remove(idx as usize);

    let mut plays = ctx
        .osu()
        .user_scores(player.user_id)
        .limit(100)
        // .offset(play as usize)
        .mode(GameMode::STD)
        .best()
        .await?;

    plays.sort_unstable_by(|a, b| {
        let a_pp = (a.pp.unwrap_or(0.0) - prev_pp).abs();
        let b_pp = (b.pp.unwrap_or(0.0) - prev_pp).abs();

        a_pp.partial_cmp(&b_pp).unwrap_or(Ordering::Equal)
    });

    let play = plays.swap_remove(play as usize);

    let map_id = play.map.as_ref().unwrap().map_id;

    let map = match ctx.psql().get_beatmap(map_id, true).await {
        Ok(map) => map,
        Err(_) => match ctx.osu().beatmap().map_id(map_id).await {
            Ok(map) => {
                // Store map in DB
                if let Err(err) = ctx.psql().insert_beatmap(&map).await {
                    warn!("{:?}", Report::new(err));
                }

                map
            }
            Err(err) => return Err(err.into()),
        },
    };

    Ok(GameStateInfo::new(player, map, play))
}
