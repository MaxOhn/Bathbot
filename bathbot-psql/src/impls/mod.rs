mod configs;
mod games;
mod huismetbenen_countries;
mod osu;
mod tracked_streams;

#[cfg(test)]
mod tests {
    pub use super::{
        configs::guild::tests::wrap_upsert_delete as guild_config_wrap_upsert_delete,
        configs::user::tests::wrap_upsert_delete as user_config_wrap_upsert_delete,
        osu::name::tests::wrap_upsert_delete as osu_name_wrap_upsert_delete,
    };
}
