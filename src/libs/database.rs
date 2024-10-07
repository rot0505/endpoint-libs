use deadpool_postgres::*;
use eyre::*;
use postgres_from_row::FromRow;
use secrecy::SecretString;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
pub use tokio_postgres::types::ToSql;
pub use tokio_postgres::Row;
mod data_thread;
mod pooled;
pub use data_thread::*;
pub use pooled::*;

use super::datatable::RDataTable;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DatabaseConfig {
    /// See [`tokio_postgres::Config::user`].
    pub user: Option<String>,
    /// See [`tokio_postgres::Config::password`].
    pub password: Option<SecretString>,
    /// See [`tokio_postgres::Config::dbname`].
    pub dbname: Option<String>,
    /// See [`tokio_postgres::Config::options`].
    pub options: Option<String>,
    /// See [`tokio_postgres::Config::application_name`].
    pub application_name: Option<String>,
    /// See [`tokio_postgres::Config::ssl_mode`].
    pub ssl_mode: Option<SslMode>,
    /// This is similar to [`Config::hosts`] but only allows one host to be
    /// specified.
    ///
    /// Unlike [`tokio_postgres::Config`] this structure differentiates between
    /// one host and more than one host. This makes it possible to store this
    /// configuration in an environment variable.
    ///
    /// See [`tokio_postgres::Config::host`].
    pub host: Option<String>,
    /// See [`tokio_postgres::Config::host`].
    pub hosts: Option<Vec<String>>,
    /// This is similar to [`Config::ports`] but only allows one port to be
    /// specified.
    ///
    /// Unlike [`tokio_postgres::Config`] this structure differentiates between
    /// one port and more than one port. This makes it possible to store this
    /// configuration in an environment variable.
    ///
    /// See [`tokio_postgres::Config::port`].
    pub port: Option<u16>,
    /// See [`tokio_postgres::Config::port`].
    pub ports: Option<Vec<u16>>,
    /// See [`tokio_postgres::Config::connect_timeout`].
    pub connect_timeout: Option<Duration>,
    /// See [`tokio_postgres::Config::keepalives`].
    pub keepalives: Option<bool>,
    /// See [`tokio_postgres::Config::keepalives_idle`].
    pub keepalives_idle: Option<Duration>,
    /// See [`tokio_postgres::Config::target_session_attrs`].
    pub target_session_attrs: Option<TargetSessionAttrs>,
    /// See [`tokio_postgres::Config::channel_binding`].
    pub channel_binding: Option<ChannelBinding>,

    /// [`Manager`] configuration.
    ///
    /// [`Manager`]: super::Manager
    pub manager: Option<ManagerConfig>,

    /// [`Pool`] configuration.
    pub pool: Option<PoolConfig>,
}

pub trait DatabaseRequest: Send {
    type ResponseRow: Send + Sync + Clone + Serialize + DeserializeOwned + FromRow;
    fn statement(&self) -> &str;
    fn params(&self) -> Vec<&(dyn ToSql + Sync)>;
}
pub type DatabaseRequestBoxed = Box<dyn DatabaseRequest<ResponseRow = Row>>;
#[derive(Clone)]
pub enum DbClient {
    Pooled(PooledDbClient),
    Threaded(ThreadedDbClient),
}
impl From<PooledDbClient> for DbClient {
    fn from(client: PooledDbClient) -> Self {
        Self::Pooled(client)
    }
}
impl From<ThreadedDbClient> for DbClient {
    fn from(value: ThreadedDbClient) -> Self {
        Self::Threaded(value)
    }
}
impl DbClient {
    pub async fn execute<T>(&self, req: T) -> Result<RDataTable<T::ResponseRow>>
    where
        T: DatabaseRequest + Sync + Send + Debug + 'static,
        T::ResponseRow: FromRow + Sync + Send + Clone + Debug + Sized + 'static,
    {
        match self {
            DbClient::Pooled(client) => client.execute(req).await,
            DbClient::Threaded(client) => client.execute(req).await,
        }
    }
}

pub fn database_test_config() -> DatabaseConfig {
    DatabaseConfig {
        user: Some("postgres".to_string()),
        password: Some("123456".to_string().into()),
        dbname: Some("red_alert".to_string()),
        host: Some("localhost".to_string()),
        ..Default::default()
    }
}

pub fn drop_and_recreate_database() -> Result<()> {
    let script = Path::new("scripts/drop_and_recreate_database.sh");
    Command::new("bash")
        .arg(script)
        .arg("etc/config.json")
        .status()?;
    Ok(())
}
