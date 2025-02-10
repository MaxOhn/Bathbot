use std::{borrow::Cow, cmp, collections::HashMap, fmt::Write, time::Duration};

use bathbot_util::{
    constants::{DESCRIPTION_SIZE, OSU_BASE},
    numbers::{round, WithComma},
    EmbedBuilder, FooterBuilder,
};
use eyre::Result;
use futures::future::BoxFuture;
use rosu_v2::model::{matches::OsuMatch, user::User};
use twilight_model::{
    channel::message::Component,
    id::{marker::UserMarker, Id},
};

use crate::{
    active::{
        pagination::{handle_pagination_component, handle_pagination_modal, Pages},
        BuildPage, ComponentResult, IActiveMessage,
    },
    commands::osu::{MatchCostDisplay, MatchResult, TeamResult, UserMatchCostEntry},
    util::interaction::{InteractionComponent, InteractionModal},
};

pub struct MatchCostPagination {
    result: MatchResult,
    osu_match: OsuMatch,
    display: MatchCostDisplay,
    content: Box<str>,
    msg_owner: Id<UserMarker>,
    pages: Pages,
}

impl IActiveMessage for MatchCostPagination {
    fn build_page(&mut self) -> BoxFuture<'_, Result<BuildPage>> {
        Box::pin(self.async_build_page())
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

    fn until_timeout(&self) -> Option<Duration> {
        match self.result {
            MatchResult::TeamVS { .. } | MatchResult::NoGames { .. } => None,
            MatchResult::HeadToHead { ref players, .. } => {
                (players.len() > self.pages.per_page()).then_some(Duration::from_secs(60))
            }
        }
    }
}

#[derive(Default)]
pub struct MatchCostPaginationBuilder {
    result: Option<MatchResult>,
    osu_match: Option<OsuMatch>,
    display: Option<MatchCostDisplay>,
    content: Option<Box<str>>,
    msg_owner: Option<Id<UserMarker>>,
}

impl MatchCostPaginationBuilder {
    pub fn build(self) -> MatchCostPagination {
        let result = self.result.expect("missing result");
        let osu_match = self.osu_match.expect("missing osu match");
        let display = self.display.expect("missing display");
        let content = self.content.expect("missing content");
        let msg_owner = self.msg_owner.expect("missing msg owner");

        let pages = match result {
            MatchResult::TeamVS { .. } => Pages::new(1, 1),
            MatchResult::HeadToHead { ref players, .. } => Pages::new(20, players.len()),
            MatchResult::NoGames { .. } => Pages::new(1, 1),
        };

        MatchCostPagination {
            result,
            osu_match,
            display,
            content,
            msg_owner,
            pages,
        }
    }

    pub fn result(mut self, result: MatchResult) -> Self {
        self.result = Some(result);

        self
    }

    pub fn osu_match(mut self, osu_match: OsuMatch) -> Self {
        self.osu_match = Some(osu_match);

        self
    }

    pub fn display(mut self, display: MatchCostDisplay) -> Self {
        self.display = Some(display);

        self
    }

    pub fn content(mut self, content: Box<str>) -> Self {
        self.content = Some(content);

        self
    }

    pub fn msg_owner(mut self, msg_owner: Id<UserMarker>) -> Self {
        self.msg_owner = Some(msg_owner);

        self
    }
}

impl MatchCostPagination {
    pub fn builder() -> MatchCostPaginationBuilder {
        MatchCostPaginationBuilder::default()
    }

    async fn async_build_page(&mut self) -> Result<BuildPage> {
        let match_id = self.osu_match.match_id;
        let mut title = self.osu_match.name.clone();
        title.retain(|c| c != '(' && c != ')');

        let mut embed = EmbedBuilder::new()
            .title(title)
            .url(format!("{OSU_BASE}community/matches/{match_id}"));

        embed = match &self.result {
            MatchResult::TeamVS {
                blue,
                red,
                mvp_avatar_url,
            } => {
                let mut description = self.description_team_vs(blue, red);

                validate_description_len(&mut description);

                embed = match self.display {
                    MatchCostDisplay::Compact => embed.thumbnail(mvp_avatar_url.as_ref()),
                    MatchCostDisplay::Full => embed.footer(FooterBuilder::new(FULL_FOOTER)),
                };

                embed.description(description)
            }
            MatchResult::HeadToHead {
                players,
                mvp_avatar_url,
            } => {
                let description = self.description_head_to_head(players);

                embed = match self.display {
                    MatchCostDisplay::Compact => embed.thumbnail(mvp_avatar_url.as_ref()),
                    MatchCostDisplay::Full => embed.footer(FooterBuilder::new(FULL_FOOTER)),
                };

                embed.description(description)
            }
            MatchResult::NoGames { description } => embed.description(description.as_str()),
        };

        Ok(BuildPage::new(embed, false).content(self.content.clone()))
    }

    fn description_team_vs(&self, blue: &TeamResult, red: &TeamResult) -> String {
        let mut description = String::new();

        fn bold(a: u32, b: u32) -> &'static str {
            if a > b {
                "**"
            } else {
                ""
            }
        }

        let _ = writeln!(
            description,
            "**{word} score**: \
            :large_blue_diamond: {blue_bold}{blue_score}{blue_bold} \
            - \
            {red_bold}{red_score}{red_bold} :small_red_triangle:\n\n\
            :large_blue_diamond: **Blue Team**",
            word = if self.osu_match.end_time.is_some() {
                "Final"
            } else {
                "Current"
            },
            blue_score = blue.win_count,
            red_score = red.win_count,
            blue_bold = bold(blue.win_count, red.win_count),
            red_bold = bold(red.win_count, blue.win_count),
        );

        let lengths = Lengths::default()
            .update(&blue.players, &self.osu_match.users)
            .update(&red.players, &self.osu_match.users);

        match self.display {
            MatchCostDisplay::Compact => {
                let medals = MedalsUserIds::new_team_vs(&blue.players, &red.players);

                fmt_compact(
                    &mut description,
                    &blue.players,
                    &lengths,
                    &self.osu_match.users,
                    &medals,
                    1,
                );

                description.push_str("\n:small_red_triangle: **Red Team**\n");

                fmt_compact(
                    &mut description,
                    &red.players,
                    &lengths,
                    &self.osu_match.users,
                    &medals,
                    1,
                );
            }
            MatchCostDisplay::Full => {
                fmt_full(
                    &mut description,
                    &blue.players,
                    &lengths,
                    &self.osu_match.users,
                    1,
                );

                description.push_str("\n:small_red_triangle: **Red Team**\n");

                fmt_full(
                    &mut description,
                    &red.players,
                    &lengths,
                    &self.osu_match.users,
                    1,
                );
            }
        }

        description
    }

    fn description_head_to_head(&self, players: &[UserMatchCostEntry]) -> String {
        let mut description = String::new();

        let lengths = Lengths::default().update(players, &self.osu_match.users);

        let idx = self.pages.index();
        let per_page = self.pages.per_page();
        let entries = &players[idx..cmp::min(players.len(), idx + per_page)];

        match self.display {
            MatchCostDisplay::Compact => {
                let medals = if idx == 0 {
                    MedalsUserIds::new_head_to_head(entries)
                } else {
                    MedalsUserIds::default()
                };

                fmt_compact(
                    &mut description,
                    entries,
                    &lengths,
                    &self.osu_match.users,
                    &medals,
                    idx + 1,
                );
            }
            MatchCostDisplay::Full => {
                fmt_full(
                    &mut description,
                    entries,
                    &lengths,
                    &self.osu_match.users,
                    idx + 1,
                );
            }
        }

        description
    }
}

const FULL_FOOTER: &str =
    "matchcost = (performance * participation * mods) + tiebreaker | average score";

#[derive(Default)]
struct Lengths {
    index: usize,
    name: usize,
    performance: usize,
    participation: usize,
    mods: usize,
    tiebreaker: usize,
    avg_score: usize,
}

impl Lengths {
    fn update(mut self, entries: &[UserMatchCostEntry], users: &HashMap<u32, User>) -> Self {
        let mut buf = String::new();

        for (entry, i) in entries.iter().zip(1..) {
            buf.clear();
            let _ = write!(buf, "{i}.");
            self.index = cmp::max(self.index, buf.len());

            buf.clear();
            let username = users.get(&entry.user_id).map(|user| user.username.as_str());

            if let Some(name) = username {
                buf.push_str(name);
            } else {
                let _ = write!(buf, "<user {}>", entry.user_id);
            }

            self.name = cmp::max(self.name, buf.len());

            buf.clear();
            let _ = write!(buf, "{}", round(entry.performance_cost));
            self.performance = cmp::max(self.performance, buf.len());

            buf.clear();
            let _ = write!(buf, "{}", round(entry.participation_bonus_factor));
            self.participation = cmp::max(self.participation, buf.len());

            buf.clear();
            let _ = write!(buf, "{}", round(entry.mods_bonus_factor));
            self.mods = cmp::max(self.mods, buf.len());

            buf.clear();
            let _ = write!(buf, "{}", round(entry.tiebreaker_bonus));
            self.tiebreaker = cmp::max(self.tiebreaker, buf.len());

            buf.clear();
            let _ = write!(buf, "{}", WithComma::new(entry.avg_score));
            self.avg_score = cmp::max(self.avg_score, buf.len());
        }

        self
    }
}

fn fmt_idx(description: &mut String, i: usize, len: usize) {
    let _ = write!(description, "`{i:<len$}");

    let dot_idx = description
        .char_indices()
        .rfind(|(_, c)| *c != ' ')
        .map_or(description.len() - 1, |(i, _)| i + 1);

    // SAFETY: the index that was just written is guaranteed to be ASCII
    //         and we just replace the first whitespace character
    {
        let bytes = unsafe { description.as_bytes_mut() };
        bytes[dot_idx] = b'.';
    }
}

fn fmt_compact(
    description: &mut String,
    players: &[UserMatchCostEntry],
    lengths: &Lengths,
    users: &HashMap<u32, User>,
    medals: &MedalsUserIds,
    start: usize,
) {
    for (entry, i) in players.iter().zip(start..) {
        fmt_idx(description, i, lengths.index);

        let _ = writeln!(
            description,
            "` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) \
                `{match_cost:0<4?}`{medal}",
            name = users.get(&entry.user_id).map_or_else(
                || format!("<user {}>", entry.user_id).into(),
                |user| Cow::Borrowed(user.username.as_str())
            ),
            name_len = lengths.name,
            user_id = entry.user_id,
            match_cost = round(entry.match_cost),
            medal = medals.get_medal(entry.user_id),
        );
    }
}

fn fmt_full(
    description: &mut String,
    players: &[UserMatchCostEntry],
    lengths: &Lengths,
    users: &HashMap<u32, User>,
    start: usize,
) {
    let mut avg_score = String::new();

    for (entry, i) in players.iter().zip(start..) {
        fmt_idx(description, i, lengths.index);

        // Formatted separately because length cannot be specified when
        // formatting WithComma
        avg_score.clear();
        let _ = write!(avg_score, "{}", WithComma::new(entry.avg_score));

        let _ = writeln!(
            description,
            "` [`{name:<name_len$}`]({OSU_BASE}u/{user_id}) \
            `{match_cost:0<4} = ({performance:>performance_len$} * \
            {participation:^participation_len$} * {mods:^mods_len$}) + \
            {tiebreaker:^tiebreaker_len$}` `{avg_score:>avg_score_len$}`",
            name = users.get(&entry.user_id).map_or_else(
                || format!("<user {}>", entry.user_id).into(),
                |user| Cow::Borrowed(user.username.as_str())
            ),
            name_len = lengths.name,
            user_id = entry.user_id,
            match_cost = round(entry.match_cost),
            performance = round(entry.performance_cost),
            performance_len = lengths.performance,
            participation = round(entry.participation_bonus_factor),
            participation_len = lengths.participation,
            mods = round(entry.mods_bonus_factor),
            mods_len = lengths.mods,
            tiebreaker = round(entry.tiebreaker_bonus),
            tiebreaker_len = lengths.tiebreaker,
            avg_score_len = lengths.avg_score,
        );
    }
}

fn validate_description_len(description: &mut String) {
    const SUFFIX: &str = "\n...";

    if description.len() > DESCRIPTION_SIZE {
        while description.len() + SUFFIX.len() > DESCRIPTION_SIZE {
            let Some((newline, _)) = description.char_indices().rfind(|(_, ch)| *ch == '\n') else {
                description.clear();
                description.push_str("Too many players, cannot display values :(");

                return;
            };

            description.truncate(newline);
        }

        description.push_str(SUFFIX);
    }
}

#[derive(Default)]
struct MedalsUserIds {
    user_ids: [u32; 3],
}

impl MedalsUserIds {
    fn new_team_vs(blue: &[UserMatchCostEntry], red: &[UserMatchCostEntry]) -> Self {
        #[derive(Copy, Clone, Default)]
        struct Value {
            user_id: u32,
            match_cost: f32,
        }
        let mut values = [Value::default(); 6];

        for (value, entry) in values[..3].iter_mut().zip(blue) {
            *value = Value {
                user_id: entry.user_id,
                match_cost: entry.match_cost,
            };
        }

        for (value, entry) in values[3..].iter_mut().zip(red) {
            *value = Value {
                user_id: entry.user_id,
                match_cost: entry.match_cost,
            };
        }

        values.sort_unstable_by(|a, b| b.match_cost.total_cmp(&a.match_cost));
        let user_ids = [values[0].user_id, values[1].user_id, values[2].user_id];

        Self { user_ids }
    }

    fn new_head_to_head(players: &[UserMatchCostEntry]) -> Self {
        let mut user_ids = [0; 3];

        for (user_id, entry) in user_ids.iter_mut().zip(players) {
            *user_id = entry.user_id;
        }

        Self { user_ids }
    }

    fn get_medal(&self, user_id: u32) -> &'static str {
        match self.user_ids.iter().position(|id| *id == user_id) {
            Some(0) => " 🥇",
            Some(1) => " 🥈",
            Some(2) => " 🥉",
            _ => "",
        }
    }
}
