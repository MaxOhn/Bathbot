mod basic_embed;
mod recent;
mod simulate;
mod util;

pub use basic_embed::BasicEmbedData;
pub use recent::RecentData;
pub use simulate::SimulateData;

use serenity::builder::CreateEmbed;

pub trait EmbedData: Send + Sync + Sized + Clone {
    fn build(self, embed: &mut CreateEmbed) -> &mut CreateEmbed;
    fn minimize<'e>(&self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        embed
    }
}

impl EmbedData for BasicEmbedData {
    fn build(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        self.build(embed)
    }
}

impl EmbedData for RecentData {
    fn build(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        self.build_embed(embed)
    }
    fn minimize<'e>(&self, embed: &'e mut CreateEmbed) -> &'e mut CreateEmbed {
        self.minimize(embed)
    }
}
