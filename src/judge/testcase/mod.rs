use async_trait::async_trait;
use std::sync::Arc;

pub struct Testcase {
    pub input: String,
    pub answer: String,
}

pub type Testcases = Vec<Testcase>;
pub type FetchResult = Result<Option<Arc<Testcases>>, String>;

#[async_trait]
pub trait TestcaseFetcher {
    fn preload(&self, id: i64);
    async fn get(&self, id: i64) -> FetchResult;
}

pub mod lru_fetcher;
