use eyre::*;
use futures::future::BoxFuture;
use std::future::Future;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio_cron_scheduler::{Job, JobScheduler};

pub struct AdaptiveJob {
    duration: Arc<RwLock<Duration>>,
    task: Box<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>,
}

impl AdaptiveJob {
    pub fn new<F>(duration: Duration, task: F) -> Self
    where
        F: Fn() -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        Self {
            duration: Arc::new(RwLock::new(duration)),
            task: Box::new(task),
        }
    }
    pub fn set_duration(&self, duration: Duration) {
        *self.duration.write().unwrap() = duration;
    }
    pub fn get_trigger(&self) -> JobTrigger {
        JobTrigger {
            duration: self.duration.clone(),
        }
    }
    pub async fn run(self) {
        loop {
            let duration = *self.duration.read().unwrap();
            tokio::time::sleep(duration).await;
            let task = (self.task)();
            tokio::spawn(task);
        }
    }
}
#[derive(Clone)]
pub struct JobTrigger {
    duration: Arc<RwLock<Duration>>,
}
impl JobTrigger {
    pub fn new(duration: Arc<RwLock<Duration>>) -> Self {
        Self { duration }
    }
    pub fn set_duration(&self, duration: Duration) {
        *self.duration.write().unwrap() = duration;
    }
}
pub struct Scheduler {
    scheduler: JobScheduler,
    pending_jobs: Vec<AdaptiveJob>,
}

impl Scheduler {
    pub async fn new() -> Self {
        Self {
            scheduler: JobScheduler::new().await.unwrap(),
            pending_jobs: vec![],
        }
    }
    pub async fn add_job<F, Fut>(&mut self, duration: Duration, f: F) -> Result<()>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future + Send + 'static,
    {
        let job = Job::new_repeated_async(duration, move |_, _| {
            let fut = f();
            Box::pin(async move {
                fut.await;
            })
        })
        .unwrap();

        self.scheduler
            .add(job)
            .await
            .map_err(|x| eyre!("{:?}", x))?;
        Ok(())
    }
    pub fn add_adaptive_job<F, Fut>(&mut self, duration: Duration, f: F) -> Result<JobTrigger>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future + Send + 'static,
    {
        let job = AdaptiveJob::new(duration, move || {
            let fut = f();
            Box::pin(async move {
                fut.await;
            })
        });
        let trigger = job.get_trigger();
        self.pending_jobs.push(job);
        Ok(trigger)
    }
    pub async fn spawn(mut self) {
        for job in self.pending_jobs.drain(..) {
            tokio::task::spawn(job.run());
        }
        self.scheduler.start().await.unwrap();
    }
}
