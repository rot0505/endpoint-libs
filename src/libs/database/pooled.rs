use dashmap::DashMap;
use deadpool_postgres::Runtime;
use deadpool_postgres::*;
use eyre::*;
use postgres_from_row::FromRow;
use secrecy::ExposeSecret;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::Hash;
use std::hash::Hasher;
use std::result::Result::Ok;
use std::sync::Arc;
use std::time::Duration;
pub use tokio_postgres::types::ToSql;
use tokio_postgres::Statement;
pub use tokio_postgres::{NoTls, Row, ToStatement};
use tracing::*;

use crate::libs::datatable::RDataTable;

use super::DatabaseConfig;
use super::DatabaseRequest;

#[derive(Clone)]
pub struct PooledDbClient {
    pool: Pool,
    prepared_stmts: Arc<DashMap<String, Statement>>,
    conn_hash: u64,
}
impl PooledDbClient {
    #[deprecated]
    pub async fn query<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, Error>
    where
        T: ?Sized + Sync + Send + ToStatement,
    {
        Ok(self
            .pool
            .get()
            .await
            .context("Failed to connect to database")?
            .query(statement, params)
            .await?)
    }

    pub async fn execute<T: DatabaseRequest + Debug>(
        &self,
        req: T,
    ) -> Result<RDataTable<T::ResponseRow>> {
        let mut error = None;
        for _ in 0..2 {
            let begin = std::time::Instant::now();
            let client = self
                .pool
                .get()
                .await
                .context("Failed to connect to database")?;
            let statement =
                tokio::time::timeout(Duration::from_secs(20), client.prepare_cached(req.statement()))
                    .await
                    .context("timeout preparing statement")??;
            let rows = match tokio::time::timeout(
                Duration::from_secs(20),
                client.query(&statement, &req.params()),
            )
            .await
            .context(format!("timeout executing statement: {}, params: {:?}", req.statement(), req.params()))?
            {
                Ok(rows) => rows,
                Err(err) => {
                    let reason = err.to_string();
                    if reason.contains("cache lookup failed for type")
                        || reason.contains("cached plan must not change result type")
                        || reason.contains("prepared statement")
                    {
                        warn!("Database has been updated. Cleaning cache and retrying query");
                        self.prepared_stmts.clear();
                        error = Some(err);
                        continue;
                    }
                    return Err(err.into());
                }
            };
            let dur = begin.elapsed();
            debug!(
                "Database query took {}.{:03} seconds: {:?}",
                dur.as_secs(),
                dur.subsec_millis(),
                req
            );
            let mut response = RDataTable::with_capacity(rows.len());
            for row in rows {
                response.push(T::ResponseRow::try_from_row(&row)?);
            }
            return Ok(response);
        }
        Err(error.unwrap().into())
    }
    pub fn conn_hash(&self) -> u64 {
        self.conn_hash
    }
}

pub async fn connect_to_database(config: DatabaseConfig) -> Result<PooledDbClient> {
    let config = Config {
        user: config.user,
        password: config.password.map(|s| s.expose_secret().clone()),
        dbname: config.dbname,
        options: config.options,
        application_name: config.application_name,
        ssl_mode: config.ssl_mode,
        host: config.host,
        hosts: config.hosts,
        port: config.port,
        ports: config.ports,
        connect_timeout: config.connect_timeout,
        keepalives: config.keepalives,
        keepalives_idle: config.keepalives_idle,
        target_session_attrs: config.target_session_attrs,
        channel_binding: config.channel_binding,
        manager: config.manager.or_else(|| {
            Some(ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            })
        }),
        pool: config.pool,
        ..Default::default()
    };
    info!(
        "Connecting to database {}:{} {}",
        config.host.as_deref().unwrap_or(""),
        config.port.unwrap_or(0),
        config.dbname.as_deref().unwrap_or("")
    );
    let mut hasher = DefaultHasher::new();
    config.host.hash(&mut hasher);
    config.port.hash(&mut hasher);
    config.dbname.hash(&mut hasher);
    let conn_hash = hasher.finish();

    let pool = config.create_pool(Some(Runtime::Tokio1), NoTls)?;
    Ok(PooledDbClient {
        pool,
        prepared_stmts: Arc::new(Default::default()),
        conn_hash,
    })
}
