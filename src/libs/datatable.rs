use std::future::Future;

use eyre::Result;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RDataTable<T> {
    rows: Vec<T>,
}

impl<T> IntoIterator for RDataTable<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}
impl<T> RDataTable<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            rows: Vec::with_capacity(capacity),
        }
    }
    pub fn first<R>(&self, f: impl Fn(&T) -> R) -> Option<R> {
        self.rows.first().map(f)
    }
    pub fn rows(&self) -> &Vec<T> {
        &self.rows
    }
    pub fn into_rows(self) -> Vec<T> {
        self.rows
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.rows.iter()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn into_result(self) -> Option<T> {
        self.rows.into_iter().next()
    }
    pub fn push(&mut self, row: T) {
        self.rows.push(row);
    }
    pub fn map<R>(self, f: impl Fn(T) -> R) -> Vec<R> {
        self.rows.into_iter().map(f).collect()
    }
    pub async fn map_async<R, F: Future<Output = Result<R>>>(self, f: impl Fn(T) -> F) -> Result<Vec<R>> {
        let mut futures = Vec::with_capacity(self.rows.len());
        for row in self.rows {
            futures.push(f(row).await?);
        }
        Ok(futures)
    }
}
