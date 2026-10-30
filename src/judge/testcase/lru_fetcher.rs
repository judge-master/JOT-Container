use crate::judge::testcase::{self, FetchResult, Testcase, TestcaseFetcher};
use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::{sync::OnceCell, time::sleep};

type CacheEntry = Arc<OnceCell<FetchResult>>;
#[derive(Clone)]
pub struct LruTestcaseFetcher {
    cache: Arc<scc::HashCache<String, CacheEntry>>,
}

impl LruTestcaseFetcher {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(scc::HashCache::new()),
        }
    }
    async fn get_e_tag(id: i64) -> Result<String, String> {
        // For now, we just return a dummy e_tag
        Ok(format!("problem-{}", id))
    }
    async fn get_testcases_from_s3(id: i64) -> FetchResult {
        // Simulate a network delay
        sleep(Duration::from_millis(50)).await;

        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/test_cases.parquet"
        ));
        let parser = testcase::parser::Parser;
        let testcases = parser
            .parse(bytes.to_vec())
            .map_err(|e| format!("Failed to parse testcases: {}", e))?;
        Ok(Some(Arc::new(testcases)))
    }
}
#[async_trait]
impl TestcaseFetcher for LruTestcaseFetcher {
    fn preload(&self, id: i64) {
        // Preload the testcases
        let self_ = self.clone();
        tokio::spawn(async move {
            let _ = self_.get(id).await;
        });
    }
    async fn get(&self, id: i64) -> FetchResult {
        let key = Self::get_e_tag(id).await?;

        let cell = self
            .cache
            .entry_async(key.clone())
            .await
            .or_put_with(|| Arc::new(OnceCell::new()))
            .1
            .clone();

        let result = cell
            .get_or_init(|| async move { Self::get_testcases_from_s3(id).await })
            .await
            .clone();

        if result.is_err() {
            // If loading failed, remove the entry from the cache after some delay
            // to prevent repeated failed attempts
            let self_ = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                self_
                    .cache
                    .remove_if_async(&key, |v| Arc::ptr_eq(v, &cell))
                    .await;
            });
        }

        result
    }
}

#[tokio::test]
async fn test_lru_testcase_fetcher() {
    let fetcher = Arc::new(LruTestcaseFetcher::new());

    // Preload a problem
    fetcher.preload(1);

    // Fetch the problem
    let result = fetcher.get(1).await;
    assert!(result.is_ok());
    let problem = result.unwrap();
    assert!(problem.is_some());
    let problem = problem.unwrap();
    // todo: assert

    // Fetch the same problem again to test caching
    let result2 = fetcher.get(1).await;
    assert!(result2.is_ok());
    let problem2 = result2.unwrap();
    assert!(problem2.is_some());
    let problem2 = problem2.unwrap();
}
