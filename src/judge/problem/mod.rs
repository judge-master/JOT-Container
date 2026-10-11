use async_trait::async_trait;
use std::sync::Arc;

pub struct Testcase {
    pub input: String,
    pub answer: String,
}

pub struct Problem {
    pub id: i64,
    pub memory_limit: u64,
    pub instruction_limit: u64,
    pub tests: Vec<Testcase>,
}

pub type FetchResult = Result<Option<Arc<Problem>>, String>;

#[async_trait]
pub trait ProblemFetcher {
    fn preload(&self, problem_id: i64);
    async fn get(&self, problem_id: i64) -> FetchResult;
}

pub mod lru_fetcher;
