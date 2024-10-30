use bathbot_cache::value::CachedArchive;
use bathbot_model::rosu_v2::user::ArchivedUser;
use bathbot_util::{constants::OSU_BASE, numbers::WithComma, osu::flag_url, AuthorBuilder};

pub trait CachedUserExt {
    fn author_builder(&self) -> AuthorBuilder;
}

impl CachedUserExt for CachedArchive<ArchivedUser> {
    fn author_builder(&self) -> AuthorBuilder {
        let stats = self.statistics.as_ref().expect("missing statistics");
        let country_code = self.country_code.as_str();

        let text = format!(
            "{name}: {pp}pp (#{global} {country_code}{national})",
            name = self.username,
            pp = WithComma::new(stats.pp.to_native()),
            global = WithComma::new(stats.global_rank.to_native()),
            national = stats.country_rank
        );

        let url = format!("{OSU_BASE}users/{}/{}", self.user_id, self.mode);
        let icon = flag_url(country_code);

        AuthorBuilder::new(text).url(url).icon_url(icon)
    }
}
