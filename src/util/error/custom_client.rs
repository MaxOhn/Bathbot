use std::{error::Error as StdError, fmt};

#[derive(Debug)]
pub enum CustomClientError {
    DataUserId,
    RankIndex(usize),
    RankingPageTable,
    RankNode(u8),
    TBody,
}

impl fmt::Display for CustomClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DataUserId => f.write_str("no attribute `data-user-id`"),
            Self::RankIndex(n) => write!(f, "expected rank between 1 and 10_000, got {}", n),
            Self::RankingPageTable => f.write_str("no class `ranking-page-table`"),
            Self::RankNode(n) => write!(f, "error at unwrap {}, expected  child", n),
            Self::TBody => f.write_str("no element `tbody`"),
        }
    }
}

impl StdError for CustomClientError {}
