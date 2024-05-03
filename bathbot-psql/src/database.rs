use eyre::Result;
use futures::{future::BoxFuture, stream::BoxStream};
use sqlx::{
    pool::PoolConnection,
    postgres::{PgPoolOptions, PgQueryResult, PgRow, PgStatement, PgTypeInfo},
    Describe, Either, Error as SqlxError, Execute, Executor, PgPool, Postgres, Transaction,
};

use crate::refresh::refresh_materialized_views;

#[derive(Debug)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub fn new(uri: &str) -> Result<Self> {
        let pool = PgPoolOptions::new().connect_lazy(uri)?;

        tokio::spawn(refresh_materialized_views(pool.clone()));

        Ok(Self { pool })
    }

    /// Retrieves a connection from the pool.
    pub(crate) async fn acquire(&self) -> Result<PoolConnection<Postgres>, SqlxError> {
        self.pool.acquire().await
    }

    /// Retrieves a connection and immediately begins a new transaction.
    pub(crate) async fn begin(&self) -> Result<Transaction<'static, Postgres>, SqlxError> {
        self.pool.begin().await
    }
}

impl<'d, 'p> Executor<'p> for &'d Database {
    type Database = Postgres;

    #[inline]
    fn fetch_many<'e, 'q, E>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<PgQueryResult, PgRow>, SqlxError>>
    where
        'q: 'e,
        'p: 'e,
        E: Execute<'q, Self::Database> + 'q,
    {
        <&PgPool as Executor<'p>>::fetch_many(&self.pool, query)
    }

    #[inline]
    fn fetch_optional<'e, 'q, E>(self, query: E) -> BoxFuture<'e, Result<Option<PgRow>, SqlxError>>
    where
        'q: 'e,
        'p: 'e,
        E: Execute<'q, Self::Database> + 'q,
    {
        <&PgPool as Executor<'p>>::fetch_optional(&self.pool, query)
    }

    #[inline]
    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [PgTypeInfo],
    ) -> BoxFuture<'e, Result<PgStatement<'q>, SqlxError>>
    where
        'p: 'e,
    {
        <&PgPool as Executor<'p>>::prepare_with(&self.pool, sql, parameters)
    }

    #[inline]
    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<Self::Database>, SqlxError>>
    where
        'p: 'e,
    {
        <&PgPool as Executor<'p>>::describe(&self.pool, sql)
    }
}
