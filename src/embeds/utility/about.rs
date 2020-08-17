use crate::{
    embeds::{EmbedData, Footer},
    format_err,
    util::{
        constants::{BATHBOT_WORKSHOP, INVITE_LINK, OWNER_USER_ID},
        datetime::how_long_ago,
        discord_avatar,
        numbers::{round, with_comma_int},
    },
    BotResult, Context,
};

use twilight_embed_builder::image_source::ImageSource;
use sysinfo::{get_current_pid, ProcessExt, ProcessorExt, System, SystemExt};

#[derive(Clone)]
pub struct AboutEmbed {
    title: String,
    thumbnail: ImageSource,
    footer: Footer,
    fields: Vec<(String, String, bool)>,
}

impl AboutEmbed {
    pub async fn new(ctx: &Context) -> BotResult<Self> {
        let owner = ctx
            .http
            .user(OWNER_USER_ID)
            .await?
            .ok_or_else(|| format_err!("Cache does not contain user of owner"))?;

        let (process_cpu, process_ram, total_cpu, used_ram, total_ram) = {
            let mut system = System::new_all();
            system.refresh_all();
            let pid = get_current_pid()
                .map_err(|why| format_err!("Could not get current PID: {}", why))?;
            let process = system
                .get_process(pid)
                .ok_or_else(|| format_err!("No process with PID {}", pid))?;
            let process_cpu = round(process.cpu_usage());
            let process_ram = process.memory() / 1000;
            let processors = system.get_processors();
            let total_cpu: f32 = round(
                processors
                    .iter()
                    .map(ProcessorExt::get_cpu_usage)
                    .sum::<f32>()
                    / processors.len() as f32,
            );
            let used_ram = (system.get_used_memory() + system.get_used_swap()) / 1000;
            let total_ram = (system.get_total_memory() + system.get_total_swap()) / 1000;
            (process_cpu, process_ram, total_cpu, used_ram, total_ram)
        };

        let bot_user = &ctx.cache.bot_user;
        let name = bot_user.name.clone();
        let shards = ctx.backend.cluster.info().len();
        let user_counts = &ctx.cache.stats.user_counts;
        let total_users = user_counts.total.get();
        let unique_users = user_counts.unique.get();
        let guild_counts = &ctx.cache.stats.guild_counts;
        let guilds = guild_counts.loaded.get() + guild_counts.outage.get();
        let channels = ctx.cache.stats.channel_count.get();

        let boot_time = ctx.cache.stats.start_time;

        let thumbnail = ImageSource::url(discord_avatar(bot_user.id, bot_user.avatar.as_deref().unwrap())).unwrap();

        let footer = Footer::new(format!("Owner: {}#{}", owner.name, owner.discriminator))
            .icon_url(discord_avatar(owner.id, owner.avatar.as_deref().unwrap()));
        let fields = vec![
            ("Guilds".to_owned(), with_comma_int(guilds as u64), true),
            (
                "Users (total)".to_owned(),
                format!(
                    "{} ({})",
                    with_comma_int(unique_users as u64),
                    with_comma_int(total_users as u64),
                ),
                true,
            ),
            ("Channels".to_owned(), with_comma_int(channels as u64), true),
            ("Shards".to_owned(), shards.to_string(), true),
            ("Process CPU".to_owned(), format!("{}%", process_cpu), true),
            ("Total CPU".to_owned(), format!("{}%", total_cpu), true),
            ("Boot time".to_owned(), how_long_ago(&boot_time), true),
            (
                "Process RAM".to_owned(),
                format!("{} MB", process_ram),
                true,
            ),
            (
                "Total RAM".to_owned(),
                format!("{}/{} MB", used_ram, total_ram),
                true,
            ),
            (
                "Github".to_owned(),
                "https://github.com/MaxOhn/Bathbot".to_string(),
                false,
            ),
            ("Invite link".to_owned(), INVITE_LINK.to_owned(), false),
            (
                "Bathbot discord server".to_owned(),
                BATHBOT_WORKSHOP.to_owned(),
                false,
            ),
        ];
        Ok(Self {
            footer,
            fields,
            thumbnail,
            title: format!("About {}", name),
        })
    }
}

impl EmbedData for AboutEmbed {
    fn title(&self) -> Option<&str> {
        Some(&self.title)
    }
    fn thumbnail(&self) -> Option<&ImageSource> {
        Some(&self.thumbnail)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
