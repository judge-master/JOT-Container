use crate::judge::testcase::{FetchResult, Testcase, TestcaseFetcher};
use async_trait::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::{sync::OnceCell, time::sleep};

type CacheEntry = Arc<OnceCell<FetchResult>>;
#[derive(Clone)]
pub struct LruTestcaseFetcher {
    cache: Arc<scc::HashCache<i64, CacheEntry>>,
}

impl LruTestcaseFetcher {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(scc::HashCache::new()),
        }
    }
}
#[async_trait]
impl TestcaseFetcher for LruTestcaseFetcher {
    fn preload(&self, problem_id: i64) {
        // Preload all testcases for the given problem
        // This is a placeholder implementation
        let self_ = self.clone();
        tokio::spawn(async move {
            let _ = self_.get(problem_id).await;
        });
    }
    async fn get(&self, problem_id: i64) -> FetchResult {
        let key = problem_id;
        // let current_cache_size = self.current_cache_size.clone();
        let cell = self
            .cache
            .entry_async(key)
            .await
            .or_put_with(|| Arc::new(OnceCell::new()))
            .1
            .clone();
        let result = cell
            .get_or_init(|| async {
                sleep(Duration::from_millis(50)).await;
                // Load the testcase from the database or filesystem
                // For now, we just return a dummy problem
                let result = Ok(Some(Arc::new(vec![
                    Testcase {
                        input: "1 2".into(),
                        answer: "3".into(),
                    },
                    Testcase {
                        input: "4 5".into(),
                        answer: "9".into(),
                    },
                ])));
                result
            })
            .await
            .clone();

        if result.is_err() {
            // If loading failed, remove the entry from the cache
            // after some delay to prevent repeated failed attempts
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
