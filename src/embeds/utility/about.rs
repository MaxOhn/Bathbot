use crate::{
    bail,
    embeds::{EmbedFields, Footer},
    util::{
        constants::{BATHBOT_WORKSHOP, INVITE_LINK, OWNER_USER_ID},
        datetime::how_long_ago_dynamic,
        discord_avatar,
        numbers::with_comma_uint,
    },
    BotResult, Context,
};

use prometheus::core::Collector;
use twilight_model::id::UserId;

pub struct AboutEmbed {
    fields: EmbedFields,
    footer: Footer,
    thumbnail: String,
    title: String,
}

impl AboutEmbed {
    pub async fn new(ctx: &Context) -> BotResult<Self> {
        let owner_id = UserId(OWNER_USER_ID);

        let owner = match ctx.cache.user(owner_id) {
            Some(user) => user,
            None => ctx.http.user(owner_id).exec().await?.model().await?,
        };

        let bot_user = match ctx.cache.current_user() {
            Some(user) => user,
            None => bail!("No CurrentUser in cache"),
        };

        let name = bot_user.name;
        let shards = ctx.cluster.info().len();
        let guilds = ctx.stats.cache_metrics.guilds.get();
        let boot_time = ctx.stats.start_time;

        let commands_used: usize = ctx.stats.command_counts.message_commands.collect()[0]
            .get_metric()
            .iter()
            .map(|metrics| metrics.get_counter().get_value() as usize)
            .sum();

        let osu_requests: usize = ctx.stats.osu_metrics.rosu.collect()[0]
            .get_metric()
            .iter()
            .map(|metric| metric.get_counter().get_value() as usize)
            .sum();

        let thumbnail = discord_avatar(bot_user.id, bot_user.avatar.as_deref().unwrap());

        let footer = Footer::new(format!("Owner: {}#{}", owner.name, owner.discriminator))
            .icon_url(discord_avatar(owner.id, owner.avatar.as_deref().unwrap()));

        let fields = vec![
            field!("Guilds", with_comma_uint(guilds as u64).to_string(), true),
            field!("Shards", shards.to_string(), true),
            field!(
                "Boot-up",
                how_long_ago_dynamic(&boot_time).to_string(),
                true
            ),
            field!(
                "Commands used",
                with_comma_uint(commands_used).to_string(),
                true
            ),
            field!(
                "osu!api requests",
                with_comma_uint(osu_requests).to_string(),
                true
            ),
            field!("Invite link", INVITE_LINK.to_owned(), false),
            field!("Bathbot discord server", BATHBOT_WORKSHOP.to_owned(), false),
            field!(
                "Github",
                "https://github.com/MaxOhn/Bathbot".to_string(),
                false
            ),
        ];

        Ok(Self {
            fields,
            footer,
            thumbnail,
            title: format!("About {}", name),
        })
    }
}

impl_builder!(AboutEmbed {
    fields,
    footer,
    thumbnail,
    title,
});
