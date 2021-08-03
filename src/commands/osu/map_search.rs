use crate::{
    arguments::{Args, MapSearchArgs},
    embeds::{EmbedData, MapSearchEmbed},
    pagination::{MapSearchPagination, Pagination},
    util::{constants::OSU_API_ISSUE, MessageExt},
    BotResult, Context,
};

use rosu_v2::prelude::{Beatmapset, BeatmapsetSearchResult, Osu, OsuResult};
use std::{collections::BTreeMap, sync::Arc};
use twilight_model::channel::Message;

#[command]
#[short_desc("Search for mapsets")]
#[long_desc(
    "Search for mapsets. \n\
    The query works essentially the same as in game, meaning you can add \
    any keywords, aswell as specific assignments like `creator=abc`, `length<123`, `ar>=9`, ...\n\n\
    Additionally, there are various special arguments you can provide with `argument=abc`:\n\
    - __`mode`__: `osu`, `taiko`, `ctb`, or `mania`, defaults to none\n\
    - __`status`__: `ranked`, `loved`, `qualified`, `pending`, `graveyard`, `any`, or \
    `leaderboard`, defaults to `leaderboard`\n\
    - __`genre`__: `any`, `unspecified`, `videogame`, `anime`, `rock`, `pop`, `other`, `novelty`, \
    `hiphop`, `electronic`, `metal`, `classical`, `folk`, or `jazz`, defaults to `any`\n\
    - __`language`__: `any`, `english`, `chinese`, `french`, `german`, `italian`, `japanese`, \
    `korean`, `spanish`, `swedish`, `russian`, `polish`, `instrumental`, `unspecified`, \
    or `other`, defaults to `any`\n\
    - __`video`__: `true` or `false`, defaults to `false`\n\
    - __`storyboard`__: `true` or `false`, defaults to `false`\n\
    - __`nsfw`__: `true` or `false`, defaults to `true` (allows nsfw, not requires nsfw)\n\
    - __`sort`__: `favourites`, `playcount`, `rankeddate`, `rating`, `relevance`, `stars`, \
    `artist`, or `title`, defaults to `relevance`\n\n\
    Depending on `sort`, the mapsets are ordered in descending order by default. \
    To reverse, specify `-asc`."
)]
#[aliases("searchmap", "mapsearch")]
#[usage("[search query]")]
#[example(
    "some words yay mode=osu status=graveyard sort=favourites -asc",
    "artist=camellia length<240 stars>8 genre=electronic"
)]
async fn search(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = match MapSearchArgs::new(args) {
        Ok(args) => args,
        Err(err_msg) => return msg.error(&ctx, err_msg).await,
    };

    let mut search_result = match args.request(ctx.osu()).await {
        Ok(response) => response,
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE);

            return Err(why.into());
        }
    };

    // Accumulate all necessary data
    let mapset_count = search_result.mapsets.len();
    let total_pages = (mapset_count < 50).then(|| mapset_count / 10 + 1);
    let maps: BTreeMap<usize, Beatmapset> = search_result.mapsets.drain(..).enumerate().collect();
    let data = MapSearchEmbed::new(&maps, &args, (1, total_pages)).await;

    // Creating the embed
    let embed = &[data.into_builder().build()];

    let response_raw = ctx
        .http
        .create_message(msg.channel_id)
        .embeds(embed)?
        .exec()
        .await?;

    // Skip pagination if too few entries
    if maps.len() <= 10 {
        return Ok(());
    }

    let response = response_raw.model().await?;

    // Pagination
    let pagination =
        MapSearchPagination::new(Arc::clone(&ctx), response, maps, search_result, args);

    let owner = msg.author.id;

    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (mapsearch): {}")
        }
    });

    Ok(())
}

impl MapSearchArgs {
    pub async fn request(&self, osu: &Osu) -> OsuResult<BeatmapsetSearchResult> {
        let mut search_fut = osu
            .beatmapset_search()
            .video(self.video)
            .storyboard(self.storyboard)
            .nsfw(self.nsfw)
            .sort(self.sort, self.descending);

        if let Some(ref query) = self.query {
            search_fut = search_fut.query(query);
        }

        if let Some(mode) = self.mode {
            search_fut = search_fut.mode(mode);
        }

        if let Some(ref status) = self.status {
            search_fut = match status.status() {
                Some(status) => search_fut.status(status),
                None => search_fut.any_status(),
            };
        }

        if let Some(genre) = self.genre {
            search_fut = search_fut.genre(genre);
        }

        if let Some(language) = self.language {
            search_fut = search_fut.language(language);
        }

        search_fut.await
    }
}
