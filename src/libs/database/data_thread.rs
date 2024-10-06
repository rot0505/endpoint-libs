use eyre::*;
use futures::future::BoxFuture;
use futures::FutureExt;
use postgres_from_row::FromRow;
use std::any::Any;
use std::fmt::Debug;

use crate::libs::datatable::RDataTable;

use super::{DatabaseRequest, PooledDbClient};

type DbExecutionRequestType =
    Box<dyn FnOnce(&PooledDbClient) -> BoxFuture<Box<dyn Any + Sync + Send>> + Send>;

struct DbExecutionQuery {
    request: DbExecutionRequestType,
    result: tokio::sync::oneshot::Sender<Box<dyn Any + Sync + Send>>,
}
#[derive(Clone)]
pub struct ThreadedDbClient {
    tx: tokio::sync::mpsc::Sender<DbExecutionQuery>,
}
impl ThreadedDbClient {
    pub async fn execute<T>(&self, req: T) -> Result<RDataTable<T::ResponseRow>>
    where
        T: DatabaseRequest + Sync + Send + Debug + 'static,
        T::ResponseRow: FromRow + Sync + Send + Clone + Debug + Sized + 'static,
    {
        let request: DbExecutionRequestType = Box::new(move |client: &PooledDbClient| {
            async move {
                let result = client.execute(req).await;
                Box::new(result) as _
            }
            .boxed()
        });
        let (tx, rx) = tokio::sync::oneshot::channel();
        let query = DbExecutionQuery {
            request,
            result: tx,
        };
        self.tx
            .send(query)
            .await
            .map_err(|_| eyre!("send failed"))?;
        let result = rx.await?;
        let result = result
            .downcast::<Result<RDataTable<T::ResponseRow>>>()
            .expect("downcast failed");
        *result
    }
}
pub fn spawn_thread_db_client(pooled: PooledDbClient) -> Result<ThreadedDbClient> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let client = ThreadedDbClient { tx };
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let client = pooled;
                while let Some(x) = rx.recv().await {
                    let DbExecutionQuery { request, result } = x;
                    let result1 = request(&client).await;
                    let _ = result.send(result1);
                }
            })
    });
    Ok(client)
}
