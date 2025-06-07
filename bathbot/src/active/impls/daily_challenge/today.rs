use std::{collections::BTreeMap, fmt::Write, mem};

use bathbot_util::{
    Authored, CowUtils, EmbedBuilder, ModsFormatter,
    constants::OSU_BASE,
    datetime::HowLongAgoDynamic,
    modal::{ModalBuilder, TextInputBuilder},
    numbers::WithComma,
};
use eyre::{Report, Result, WrapErr};
use time::{Date, Duration, Month, UtcDateTime};
use twilight_model::{
    channel::message::{
        Component, EmojiReactionType,
        component::{ActionRow, Button, ButtonStyle},
    },
    id::{Id, marker::UserMarker},
};

use crate::{
    active::{BuildPage, ComponentResult, IActiveMessage, pagination::Pages},
    commands::osu::DailyChallengeDay,
    util::{
        ComponentExt, Emote,
        interaction::{InteractionComponent, InteractionModal},
        osu::GradeFormatter,
    },
};

pub struct DailyChallengeTodayPagination {
    days: BTreeMap<usize, PaginatedDay>,
    osu_id: Option<u32>,
    msg_owner: Id<UserMarker>,
    day_pages: Pages,
    defer: bool,
    disable_last: bool,
}

struct PaginatedDay {
    data: DailyChallengeDay,
    pages: Pages,
}

impl PaginatedDay {
    fn new(data: DailyChallengeDay) -> Self {
        Self {
            pages: Pages::new(10, data.leaderboard.len()),
            data,
        }
    }
}

impl DailyChallengeTodayPagination {
    pub fn new(osu_id: Option<u32>, today: DailyChallengeDay, msg_owner: Id<UserMarker>) -> Self {
        let mut days = BTreeMap::new();
        days.insert(0, PaginatedDay::new(today));

        Self {
            days,
            osu_id,
            msg_owner,
            day_pages: Pages::new(1, usize::MAX),
            defer: false,
            disable_last: false,
        }
    }

    async fn handle_component_raw(
        &mut self,
        component: &mut InteractionComponent,
    ) -> Result<ComponentResult> {
        if component.user_id()? != self.msg_owner {
            return Ok(ComponentResult::Ignore);
        }

        match component.data.custom_id.as_str() {
            "pagination_start" => {
                let Some(prev_idx) = self.day_pages.index().checked_sub(1) else {
                    return Ok(ComponentResult::Ignore);
                };

                self.populate_idx(component, prev_idx).await?;
            }
            "pagination_back" => {
                if let Some(day) = self.days.get_mut(&self.day_pages.index()) {
                    day.pages
                        .set_index(day.pages.index().saturating_sub(day.pages.per_page()));
                }
            }
            "pagination_step" => {
                if let Some(day) = self.days.get_mut(&self.day_pages.index()) {
                    day.pages
                        .set_index(day.pages.index() + day.pages.per_page());
                }
            }
            "pagination_end" => {
                self.populate_idx(component, self.day_pages.index() + 1)
                    .await?;
            }
            "pagination_custom" => {
                let day_placeholder = "Date of the form YYYY-MM-DD".to_owned();

                let day_input = TextInputBuilder::new("day_page_input", "Date")
                    .min_len(10) // yyyy-mm-dd
                    .max_len(10)
                    .placeholder(day_placeholder)
                    .required(false);

                let max_page = self.days[&self.day_pages.index()].pages.last_page();
                let leaderboard_placeholder = format!("Number between 1 and {max_page}");

                let leaderboard_input =
                    TextInputBuilder::new("lb_page_input", "Leaderboard page number")
                        .min_len(1)
                        .max_len(1)
                        .placeholder(leaderboard_placeholder)
                        .required(false);

                let modal = ModalBuilder::new("pagination_page", "Jump to a page")
                    .input(leaderboard_input)
                    .input(day_input);

                return Ok(ComponentResult::CreateModal(modal));
            }
            other => {
                warn!(name = %other, ?component, "Unknown pagination component");

                return Ok(ComponentResult::Ignore);
            }
        }

        Ok(ComponentResult::BuildPage)
    }

    async fn populate_idx(
        &mut self,
        component: &mut InteractionComponent,
        idx: usize,
    ) -> Result<()> {
        if self.days.contains_key(&idx) {
            if let Some(day) = self.days.get_mut(&self.day_pages.index()) {
                day.pages.set_index(0);
            }

            self.day_pages.set_index(idx);

            return Ok(());
        }

        component
            .defer()
            .await
            .wrap_err("Failed to defer component")?;

        self.defer = true;

        let date = idx_to_date(idx);

        match DailyChallengeDay::new(self.osu_id, date).await {
            Ok(day) => {
                if let Some(day) = self.days.get_mut(&self.day_pages.index()) {
                    day.pages.set_index(0);
                }

                self.day_pages.set_index(idx);
                self.days.insert(idx, PaginatedDay::new(day));
            }
            Err(err) => {
                self.disable_last = true;

                return Err(err);
            }
        }

        Ok(())
    }
}

impl IActiveMessage for DailyChallengeTodayPagination {
    async fn build_page(&mut self) -> Result<BuildPage> {
        let day_idx = self.day_pages.index();
        let PaginatedDay { data: day, pages } = &self.days[&day_idx];

        let start_idx = pages.index();
        let end_idx = day.leaderboard.len().min(start_idx + pages.per_page());

        let mut description = String::with_capacity(1024);

        for (i, item) in day.leaderboard[start_idx..end_idx].iter().enumerate() {
            let Some(score) = day.scores.get(&item.user_id) else {
                warn!(user_id = item.user_id, "Missing score for leaderboard item");

                continue;
            };

            let _ = writeln!(
                description,
                "**#{i}** **[{username}]({OSU_BASE}users/{user_id})**: \
                {score} **+{mods}**\n{grade} {attempts} attempt{plural} • {acc:.2}% {ago}",
                i = start_idx + i + 1,
                username = item.user.username,
                user_id = item.user.user_id,
                score = WithComma::new(item.score),
                mods = ModsFormatter::new(&score.mods, false),
                grade = GradeFormatter::new(score.grade, Some(score.id), false),
                attempts = item.attempts,
                plural = if item.attempts != 1 { "s" } else { "" },
                acc = item.accuracy,
                ago = HowLongAgoDynamic::new(&score.ended_at),
            );
        }

        if day.leaderboard.is_empty() {
            description.push_str("No scores yet\n");
        }

        description.push_str(&day.description);

        let title = format!(
            "{} - {} [{}]",
            day.map.artist().cow_escape_markdown(),
            day.map.title().cow_escape_markdown(),
            day.map.version().cow_escape_markdown(),
        );

        let url = format!("{OSU_BASE}b/{}", day.map.map_id());

        let embed = EmbedBuilder::new()
            .author(day.author.clone())
            .description(description)
            .footer(day.footer.clone())
            .image(day.map.cover())
            .timestamp(day.start_time)
            .title(title)
            .url(url);

        Ok(BuildPage::new(embed, mem::replace(&mut self.defer, false)))
    }

    fn build_components(&self) -> Vec<Component> {
        let day_pages = &self.day_pages;
        let leaderboard_pages = &self.days[&day_pages.index()].pages;

        let jump_start = Button {
            custom_id: Some("pagination_start".to_owned()),
            disabled: day_pages.index() == 0,
            emoji: Some(EmojiReactionType::Unicode {
                name: "⏪".to_owned(),
            }),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step_back = Button {
            custom_id: Some("pagination_back".to_owned()),
            disabled: leaderboard_pages.index() == 0,
            emoji: Some(Emote::SingleStepBack.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let jump_custom = Button {
            custom_id: Some("pagination_custom".to_owned()),
            disabled: false,
            emoji: Some(Emote::MyPosition.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let single_step = Button {
            custom_id: Some("pagination_step".to_owned()),
            disabled: leaderboard_pages.index() == leaderboard_pages.last_index(),
            emoji: Some(Emote::SingleStep.reaction_type()),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
        };

        let jump_end = Button {
            custom_id: Some("pagination_end".to_owned()),
            disabled: self.disable_last,
            emoji: Some(EmojiReactionType::Unicode {
                name: "⏩".to_owned(),
            }),
            label: None,
            style: ButtonStyle::Secondary,
            url: None,
            sku_id: None,
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

    async fn handle_component(&mut self, component: &mut InteractionComponent) -> ComponentResult {
        self.handle_component_raw(component)
            .await
            .unwrap_or_else(ComponentResult::Err)
    }

    async fn handle_modal(&mut self, modal: &mut InteractionModal) -> Result<()> {
        if modal.user_id()? != self.msg_owner {
            return Ok(());
        }

        let day_page_input = modal
            .data
            .components
            .iter()
            .flat_map(|row| row.components.iter())
            .find(|component| component.custom_id == "day_page_input");

        if let Some(input) = day_page_input {
            let value = input.value.as_deref().unwrap_or_default();

            let mut iter = value.split('-').map(str::parse::<u16>);

            let year = iter.next();
            let month = iter
                .next()
                .map(|res| res.map(|month| Month::try_from(month as u8)));
            let day = iter.next();

            let (year, month, day) = match (year, month, day) {
                (Some(Ok(year)), Some(Ok(Ok(month))), Some(Ok(day))) => (year, month, day),
                _ => {
                    debug!(input = input.value, "Failed to parse date input");

                    return Ok(());
                }
            };

            let date = match Date::from_calendar_date(year as i32, month, day as u8) {
                Ok(date) => date,
                Err(err) => {
                    debug!(input = input.value, err = ?Report::new(err), "Invalid date input");

                    return Ok(());
                }
            };

            let today = UtcDateTime::now().date();
            let first = DailyChallengeDay::FIRST_DATE;

            if !(first..=today).contains(&date) {
                debug!("No daily challenge for {date}");

                return Ok(());
            }

            let idx = date_to_idx(date);

            if !self.days.contains_key(&idx) {
                let day = DailyChallengeDay::new(self.osu_id, date).await?;
                self.days.insert(idx, PaginatedDay::new(day));
            }

            self.day_pages.set_index(idx);

            return Ok(());
        }

        let leaderboard_page_input = modal
            .data
            .components
            .iter()
            .flat_map(|row| row.components.iter())
            .find(|component| component.custom_id == "lb_page_input");

        if let Some(input) = leaderboard_page_input {
            let Some(Ok(page)) = input.value.as_deref().map(str::parse) else {
                debug!(input = input.value, "Failed to parse page input as usize");

                return Ok(());
            };

            let pages = &mut self.days.get_mut(&self.day_pages.index()).unwrap().pages;
            let max_page = pages.last_page();

            if !(1..=max_page).contains(&page) {
                debug!("Page {page} is not between 1 and {max_page}");

                return Ok(());
            }

            pages.set_index((page - 1) * pages.per_page());

            return Ok(());
        }

        Ok(())
    }
}

// today yesterday ...
//   0       1
fn date_to_idx(date: Date) -> usize {
    i64::max(0, (UtcDateTime::now().date() - date).whole_days()) as usize
}

// [today, yesterday, ...]
fn idx_to_date(idx: usize) -> Date {
    UtcDateTime::now().date() - Duration::days(idx as i64)
}
