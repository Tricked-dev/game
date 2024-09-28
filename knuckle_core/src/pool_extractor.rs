use crate::internal_error;
use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use bb8::{Pool, PooledConnection};
use bb8_postgres::PostgresConnectionManager;
use http::{request::Parts, StatusCode};
use tokio_postgres::NoTls;

pub type ConnectionPool = Pool<PostgresConnectionManager<NoTls>>;

pub type Conn = PooledConnection<'static, PostgresConnectionManager<NoTls>>;

pub struct DatabaseConnection(pub Conn);

#[async_trait]
impl<S> FromRequestParts<S> for DatabaseConnection
where
    ConnectionPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        _parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let pool = ConnectionPool::from_ref(state);

        let conn = pool.get_owned().await.map_err(internal_error)?;

        Ok(Self(conn))
    }
}
