use crate::{
    embeds::{EmbedData, Footer},
    util::{
        datetime::how_long_ago,
        numbers::{round, with_comma_u64},
    },
    BootTime, Error,
};

use serenity::client::Context;
use sysinfo::{get_current_pid, ProcessExt, ProcessorExt, System, SystemExt};

#[derive(Clone)]
pub struct AboutEmbed {
    title: String,
    thumbnail: String,
    footer: Footer,
    fields: Vec<(String, String, bool)>,
}

impl AboutEmbed {
    pub async fn new(ctx: &Context) -> Result<Self, Error> {
        let owner = ctx.http.get_current_application_info().await?.owner;

        let (process_cpu, process_ram, total_cpu, used_ram, total_ram) = {
            let mut system = System::new_all();
            system.refresh_all();
            let pid = get_current_pid().unwrap();
            let process = system.get_process(pid).unwrap();
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

        let cache = &ctx.cache;
        let name = cache.current_user_field(|user| user.name.clone()).await;
        let shards = cache.shard_count().await.to_string();
        let users = with_comma_u64(cache.user_count().await as u64);
        let guilds = with_comma_u64(cache.guild_count().await as u64);
        let channels = with_comma_u64(cache.guild_channel_count().await as u64);

        let data = ctx.data.read().await;
        let boot_time = *data.get::<BootTime>().unwrap();

        let thumbnail = cache
            .current_user_field(|user| user.avatar_url())
            .await
            .unwrap();
        let footer =
            Footer::new(format!("Owner: {}", owner.tag())).icon_url(owner.avatar_url().unwrap());
        let fields = vec![
            ("Guilds".to_owned(), guilds, true),
            ("Users".to_owned(), users, true),
            ("Channels".to_owned(), channels, true),
            ("Shards".to_owned(), shards, true),
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
            (
                "Invite link".to_owned(),
                "https://discordapp.com/api/oauth2/authorize?scope=bot&\
                client_id=297073686916366336&permissions=268823616"
                    .to_string(),
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
    fn thumbnail(&self) -> Option<&str> {
        Some(&self.thumbnail)
    }
    fn footer(&self) -> Option<&Footer> {
        Some(&self.footer)
    }
    fn fields(&self) -> Option<Vec<(String, String, bool)>> {
        Some(self.fields.clone())
    }
}
