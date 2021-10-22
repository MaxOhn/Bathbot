use crate::{
    bg_game::MapsetTags,
    database::{util::CustomSQL, MapsetTagWrapper, TagRow},
    BotResult, Database,
};

use rosu_v2::model::GameMode;
use tokio_stream::StreamExt;

struct StatsEntry {
    discord_id: i64,
    score: i32,
}

impl Database {
    pub async fn increment_bggame_score(&self, user_id: u64, amount: i32) -> BotResult<()> {
        sqlx::query!(
            "INSERT INTO bggame_scores \
            VALUES ($1,$2) ON CONFLICT (discord_id) DO \
            UPDATE \
            SET score=bggame_scores.score+$2",
            user_id as i64,
            amount
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn all_bggame_scores(&self) -> BotResult<Vec<(u64, u32)>> {
        let scores = sqlx::query_as!(StatsEntry, "SELECT * FROM bggame_scores")
            .fetch(&self.pool)
            .map(|res| res.map(|entry| (entry.discord_id as u64, entry.score as u32)))
            .collect::<Result<_, _>>()
            .await?;

        Ok(scores)
    }

    pub async fn add_tag_mapset(
        &self,
        mapset_id: u32,
        filename: &str,
        mode: GameMode,
    ) -> BotResult<()> {
        sqlx::query!(
            "INSERT INTO map_tags (mapset_id,filename,mode) VALUES ($1,$2,$3)",
            mapset_id as i32,
            filename,
            mode as i16
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_tags_mapset(&self, mapset_id: u32, tags: MapsetTags) -> BotResult<()> {
        sqlx::query!(
            "UPDATE map_tags SET \
            farm=map_tags.farm OR $1,\
            streams=map_tags.streams OR $2,\
            alternate=map_tags.alternate OR $3,\
            old=map_tags.old OR $4,\
            meme=map_tags.meme OR $5,\
            hardname=map_tags.hardname OR $6,\
            easy=map_tags.easy OR $7,\
            hard=map_tags.hard OR $8,\
            tech=map_tags.tech OR $9,\
            weeb=map_tags.weeb OR $10,\
            bluesky=map_tags.bluesky OR $11,\
            english=map_tags.english OR $12,\
            kpop=map_tags.kpop OR $13 \
            WHERE mapset_id=$14",
            tags.contains(MapsetTags::Farm),
            tags.contains(MapsetTags::Streams),
            tags.contains(MapsetTags::Alternate),
            tags.contains(MapsetTags::Old),
            tags.contains(MapsetTags::Meme),
            tags.contains(MapsetTags::HardName),
            tags.contains(MapsetTags::Easy),
            tags.contains(MapsetTags::Hard),
            tags.contains(MapsetTags::Tech),
            tags.contains(MapsetTags::Weeb),
            tags.contains(MapsetTags::BlueSky),
            tags.contains(MapsetTags::English),
            tags.contains(MapsetTags::Kpop),
            mapset_id as i32,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_tags_mapset(&self, mapset_id: u32, tags: MapsetTags) -> BotResult<()> {
        sqlx::query!(
            "UPDATE map_tags SET \
            farm=map_tags.farm AND $1,\
            streams=map_tags.streams AND $2,\
            alternate=map_tags.alternate AND $3,\
            old=map_tags.old AND $4,\
            meme=map_tags.meme AND $5,\
            hardname=map_tags.hardname AND $6,\
            easy=map_tags.easy AND $7,\
            hard=map_tags.hard AND $8,\
            tech=map_tags.tech AND $9,\
            weeb=map_tags.weeb AND $10,\
            bluesky=map_tags.bluesky AND $11,\
            english=map_tags.english AND $12,\
            kpop=map_tags.kpop AND $13 \
            WHERE mapset_id=$14",
            !tags.contains(MapsetTags::Farm),
            !tags.contains(MapsetTags::Streams),
            !tags.contains(MapsetTags::Alternate),
            !tags.contains(MapsetTags::Old),
            !tags.contains(MapsetTags::Meme),
            !tags.contains(MapsetTags::HardName),
            !tags.contains(MapsetTags::Easy),
            !tags.contains(MapsetTags::Hard),
            !tags.contains(MapsetTags::Tech),
            !tags.contains(MapsetTags::Weeb),
            !tags.contains(MapsetTags::BlueSky),
            !tags.contains(MapsetTags::English),
            !tags.contains(MapsetTags::Kpop),
            mapset_id as i32,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_tags_mapset(&self, mapset_id: u32) -> BotResult<MapsetTagWrapper> {
        let tags = sqlx::query_as!(
            TagRow,
            "SELECT * FROM map_tags WHERE mapset_id=$1 LIMIT 1",
            mapset_id as i32
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(tags.into())
    }

    pub async fn get_all_tags_mapset(&self, mode: GameMode) -> BotResult<Vec<MapsetTagWrapper>> {
        let tags = sqlx::query_as!(TagRow, "SELECT * FROM map_tags WHERE mode=$1", mode as i16)
            .fetch(&self.pool)
            .map(|res| res.map(|row| row.into()))
            .collect::<Result<_, _>>()
            .await?;

        Ok(tags)
    }

    pub async fn get_random_tags_mapset(&self, mode: GameMode) -> BotResult<MapsetTagWrapper> {
        let tags = sqlx::query_as!(
            TagRow,
            "SELECT * FROM map_tags WHERE mode=$1 ORDER BY RANDOM() LIMIT 1",
            mode as i16
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(tags.into())
    }

    pub async fn get_specific_tags_mapset(
        &self,
        mode: GameMode,
        included: MapsetTags,
        excluded: MapsetTags,
    ) -> BotResult<Vec<MapsetTagWrapper>> {
        if included.is_empty() && excluded.is_empty() {
            return self.get_all_tags_mapset(mode).await;
        }

        let mut query = String::from("SELECT * FROM map_tags WHERE mode=$1 AND");

        if !included.is_empty() {
            query = query.set_tags(" AND ", included, true)?;

            if !excluded.is_empty() {
                query.push_str(" AND");
            }
        }

        if !excluded.is_empty() {
            query = query.set_tags(" AND ", excluded, false)?;
        }

        let tags = sqlx::query_as(&query)
            .bind(mode as i16)
            .fetch_all(&self.pool)
            .await?;

        Ok(tags)
    }
}
