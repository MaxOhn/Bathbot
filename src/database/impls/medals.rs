use crate::{custom_client::OsekaiMedal, BotResult, Database};

use futures::stream::StreamExt;
use hashbrown::HashMap;
use rosu_v2::prelude::GameMods;

impl Database {
    pub async fn store_medals(&self, medals: &[OsekaiMedal]) -> BotResult<()> {
        for medal in medals {
            self._store_medal(medal).await?;
        }

        Ok(())
    }

    async fn _store_medal(&self, medal: &OsekaiMedal) -> BotResult<()> {
        let query = sqlx::query!(
            "INSERT INTO osekai_medals\
            (medal_id,name,icon_url,description,restriction,grouping,solution,mods,mode_order,ordering) VALUES\
            ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT (medal_id) DO UPDATE SET medal_id=$1,name=$2,icon_url=$3,\
            description=$4,restriction=$5,grouping=$6,solution=$7,mods=$8,mode_order=$9,ordering=$10",
            medal.medal_id as i32,
            medal.name,
            medal.icon_url,
            medal.description,
            medal.restriction.map(|m| m as i16),
            medal.grouping,
            medal.solution,
            medal.mods.map(|m| m.bits() as i32),
            medal.mode_order as i64,
            medal.ordering as i64,
        );

        query.execute(&self.pool).await?;

        Ok(())
    }

    pub async fn get_medal_by_id(&self, medal_id: u32) -> BotResult<Option<OsekaiMedal>> {
        let query = sqlx::query!(
            "SELECT * FROM osekai_medals WHERE medal_id=$1",
            medal_id as i32
        );

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => Ok(Some(OsekaiMedal {
                medal_id: entry.medal_id as u32,
                name: entry.name,
                icon_url: entry.icon_url,
                description: entry.description,
                restriction: entry.restriction.map(|m| (m as u8).into()),
                grouping: entry.grouping,
                solution: entry.solution,
                mods: entry.mods.and_then(|m| GameMods::from_bits(m as u32)),
                mode_order: entry.mode_order as usize,
                ordering: entry.ordering as usize,
            })),
            None => Ok(None),
        }
    }

    pub async fn get_medal_by_name(&self, name: &str) -> BotResult<Option<OsekaiMedal>> {
        let query = sqlx::query!("SELECT * FROM osekai_medals WHERE name ILIKE $1", name);

        match query.fetch_optional(&self.pool).await? {
            Some(entry) => Ok(Some(OsekaiMedal {
                medal_id: entry.medal_id as u32,
                name: entry.name,
                icon_url: entry.icon_url,
                description: entry.description,
                restriction: entry.restriction.map(|m| (m as u8).into()),
                grouping: entry.grouping,
                solution: entry.solution,
                mods: entry.mods.and_then(|m| GameMods::from_bits(m as u32)),
                mode_order: entry.mode_order as usize,
                ordering: entry.ordering as usize,
            })),
            None => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub async fn get_medals_by_name_infix(&self, name: &str) -> BotResult<Vec<OsekaiMedal>> {
        let pattern = format!("%{}%", name);
        let mut stream = sqlx::query!("SELECT * FROM osekai_medals WHERE name ILIKE $1", pattern)
            .fetch(&self.pool);

        let mut medals = Vec::new();

        while let Some(entry) = stream.next().await.transpose()? {
            let medal = OsekaiMedal {
                medal_id: entry.medal_id as u32,
                name: entry.name,
                icon_url: entry.icon_url,
                description: entry.description,
                restriction: entry.restriction.map(|m| (m as u8).into()),
                grouping: entry.grouping,
                solution: entry.solution,
                mods: entry.mods.and_then(|m| GameMods::from_bits(m as u32)),
                mode_order: entry.mode_order as usize,
                ordering: entry.ordering as usize,
            };

            medals.push(medal);
        }

        Ok(medals)
    }

    pub async fn get_medals(&self) -> BotResult<HashMap<u32, OsekaiMedal>> {
        let mut stream = sqlx::query!("SELECT * FROM osekai_medals").fetch(&self.pool);
        let mut medals = HashMap::with_capacity(257);

        while let Some(entry) = stream.next().await.transpose()? {
            let medal = OsekaiMedal {
                medal_id: entry.medal_id as u32,
                name: entry.name,
                icon_url: entry.icon_url,
                description: entry.description,
                restriction: entry.restriction.map(|m| (m as u8).into()),
                grouping: entry.grouping,
                solution: entry.solution,
                mods: entry.mods.and_then(|m| GameMods::from_bits(m as u32)),
                mode_order: entry.mode_order as usize,
                ordering: entry.ordering as usize,
            };

            medals.insert(medal.medal_id, medal);
        }

        Ok(medals)
    }

    pub async fn get_medal_names(&self) -> BotResult<Vec<String>> {
        let mut stream = sqlx::query!("SELECT name FROM osekai_medals").fetch(&self.pool);
        let mut names = Vec::with_capacity(257);

        while let Some(entry) = stream.next().await.transpose()? {
            names.push(entry.name);
        }

        Ok(names)
    }
}
