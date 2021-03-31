use crate::{database::Prefix, embeds::EmbedFields};

pub struct BGHelpEmbed {
    description: &'static str,
    fields: EmbedFields,
    title: &'static str,
}

impl BGHelpEmbed {
    pub fn new(prefix: Prefix) -> Self {
        let description = "Given part of a map's background, \
            try to guess the **title** of the map's song.\n\
            You don't need to guess content in parentheses `(...)` \
            or content after `ft.` or `feat.`.\n\
            Use these subcommands to initiate with the game:";

        let fields = vec![
            field!(
                "start / s / skip / resolve / r",
                format!(
                    "__If no game is running yet:__\n\
                    Start the game in the current channel.\n\
                    First, you get to choose which kind of backgrounds \
                    you will need to guess.\n\
                    React to require a tag, or react-unreact to exclude a tag.\n\
                    If no tag is chosen, all backgrounds will be selected.\n\
                    __If the game is already going:__\n\
                    Resolve the current background and give a new one \
                    with the same tag specs.\n\
                    To change mode or tags, be sure to `{prefix}bg stop` first.\n\
                    __Mania:__\n\
                    *Start* the game with the additional argument \
                    `mania` or just `m` e.g. `{prefix}bg s m`. \
                    Once the mania game is running, you can skip with `{prefix}bg s`.",
                    prefix = prefix
                ),
                false
            ),
            field!(
                "hint / h / tip",
                "Receive a hint (can be used multiple times)".to_owned(),
                true
            ),
            field!(
                "bigger / b / enhance",
                "Increase the radius of the displayed image \
                (can be used multiple times)"
                    .to_owned(),
                true
            ),
            field!(
                "stop / end / quit",
                "Resolve the current background and stop the game in this channel".to_owned(),
                true
            ),
            field!(
                "ranking / leaderboard / lb / stats",
                format!(
                    "Check out the global leaderboard for amount of correct guesses.\n\
                    If you add `server` or `s` at the end, e.g. `{prefix}bg lb s`, \
                    I will only consider members of the server.",
                    prefix = prefix
                ),
                false
            ),
        ];

        Self {
            description,
            fields,
            title: "Background guessing game",
        }
    }
}

impl_builder!(BGHelpEmbed {
    description,
    fields,
    title,
});
