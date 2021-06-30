use crate::{
    bail,
    embeds::{EmbedFields, Footer},
    format_err,
    util::{
        constants::{BATHBOT_WORKSHOP, INVITE_LINK, OWNER_USER_ID},
        discord_avatar,
        numbers::with_comma_uint,
    },
    BotResult, Context,
};

use chrono::{DateTime, Utc};
use sysinfo::{get_current_pid, ProcessExt, ProcessorExt, RefreshKind, System, SystemExt};
use twilight_model::id::UserId;

pub struct AboutEmbed {
    fields: EmbedFields,
    footer: Footer,
    thumbnail: String,
    timestamp: DateTime<Utc>,
    title: String,
}

impl AboutEmbed {
    pub async fn new(ctx: &Context) -> BotResult<Self> {
        let owner = ctx
            .http
            .user(UserId(OWNER_USER_ID))
            .await?
            .ok_or_else(|| format_err!("Cache does not contain user of owner"))?;

        let refreshes = RefreshKind::new().with_cpu().with_memory().with_processes();
        let mut system = System::new_with_specifics(refreshes);

        let (process_ram, total_cpu, used_ram, total_ram) = {
            system.refresh_specifics(refreshes);

            let pid = get_current_pid()
                .map_err(|why| format_err!("Could not get current PID: {}", why))?;

            let process = system
                .process(pid)
                .ok_or_else(|| format_err!("No process with PID {}", pid))?;

            let process_ram = process.memory() / 1000;
            let processors = system.processors();
            let processor_count = processors.len() as f32;

            let total_cpu: f32 =
                processors.iter().map(ProcessorExt::cpu_usage).sum::<f32>() / processor_count;

            let used_ram = (system.used_memory() + system.used_swap()) / 1000;
            let total_ram = (system.total_memory() + system.total_swap()) / 1000;

            (process_ram, total_cpu, used_ram, total_ram)
        };

        let bot_user = match ctx.cache.current_user() {
            Some(user) => user,
            None => bail!("No CurrentUser in cache"),
        };

        let name = bot_user.name.clone();
        let shards = ctx.backend.cluster.info().len();
        let guilds = ctx.stats.cache_metrics.guilds.get();

        let boot_time = ctx.stats.start_time;

        let thumbnail = discord_avatar(bot_user.id, bot_user.avatar.as_deref().unwrap());

        let footer = Footer::new(format!(
            "Owner: {}#{} | Boot time",
            owner.name, owner.discriminator
        ))
        .icon_url(discord_avatar(owner.id, owner.avatar.as_deref().unwrap()));

        let fields = vec![
            field!("Guilds", with_comma_uint(guilds as u64).to_string(), true),
            field!("Total RAM", format!("{}/{} MB", used_ram, total_ram), true),
            field!("Process RAM", format!("{} MB", process_ram), true),
            field!("Shards", shards.to_string(), true),
            field!("Total CPU", format!("{:.2}%", total_cpu), true),
            field!(
                "Github",
                "https://github.com/MaxOhn/Bathbot".to_string(),
                false
            ),
            field!("Invite link", INVITE_LINK.to_owned(), false),
            field!("Bathbot discord server", BATHBOT_WORKSHOP.to_owned(), false),
        ];

        Ok(Self {
            fields,
            footer,
            thumbnail,
            timestamp: boot_time,
            title: format!("About {}", name),
        })
    }
}

impl_builder!(AboutEmbed {
    fields,
    footer,
    thumbnail,
    timestamp,
    title,
});
