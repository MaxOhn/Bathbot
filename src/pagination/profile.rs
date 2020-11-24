use super::{MainReactions, Pages, Pagination};

use crate::{commands::osu::profile_embed, embeds::ProfileEmbed, BotResult, Context, Error};

use async_trait::async_trait;
use rosu::model::GameMode;
use std::{collections::HashMap, sync::Arc};
use twilight_http::request::channel::reaction::RequestReactionType;
use twilight_model::channel::Message;

pub struct ProfilePagination {
    msg: Message,
    pages: Pages,
    embeds: HashMap<usize, ProfileEmbed>,
    name: String,
    ctx: Arc<Context>,
}

impl ProfilePagination {
    pub fn new(
        ctx: Arc<Context>,
        msg: Message,
        mode: GameMode,
        name: String,
        embed: ProfileEmbed,
    ) -> Self {
        let mut embeds = HashMap::with_capacity(1);
        embeds.insert(mode as usize, embed);
        let mut pages = Pages::new(1, 4);
        pages.index = mode as usize;
        Self {
            msg,
            pages,
            embeds,
            name,
            ctx,
        }
    }
}

#[async_trait]
impl Pagination for ProfilePagination {
    type PageData = ProfileEmbed;
    fn msg(&self) -> &Message {
        &self.msg
    }
    fn pages(&self) -> Pages {
        self.pages
    }
    fn pages_mut(&mut self) -> &mut Pages {
        &mut self.pages
    }
    fn reactions() -> Vec<RequestReactionType> {
        Self::mode_reactions()
    }
    fn main_reactions(&self) -> MainReactions {
        MainReactions::Modes
    }
    async fn change_mode(&mut self) {
        let mode = GameMode::from(self.pages.index as u8);
        #[allow(clippy::clippy::map_entry)]
        if !self.embeds.contains_key(&self.pages.index) {
            let profile_fut = profile_embed(&self.ctx, &self.name, mode, self.msg());
            if let Ok(Some((data, _))) = profile_fut.await {
                self.embeds.insert(self.pages.index, data);
            }
        }
    }
    async fn build_page(&mut self) -> BotResult<Self::PageData> {
        match self.embeds.get(&self.pages.index) {
            Some(embed) => Ok(embed.to_owned()),
            None => {
                let content = format!("gamemode {} was unavailable", self.pages.index);
                Err(Error::Custom(content))
            }
        }
    }
}
