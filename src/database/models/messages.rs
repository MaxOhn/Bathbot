use super::super::schema::messages;
use chrono::NaiveDateTime;

#[derive(Queryable, Insertable, Debug)]
#[table_name = "messages"]
pub struct InsertableMessage {
    pub id: u64,
    pub channel_id: u64,
    pub author: u64,
    pub content: String,
    pub timestamp: NaiveDateTime,
}
