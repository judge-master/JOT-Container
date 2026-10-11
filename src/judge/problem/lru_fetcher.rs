use crate::judge::problem::{FetchResult, Problem, ProblemFetcher, Testcase};
use async_trait::async_trait;
use std::{
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
    time::Duration,
};
use tokio::{sync::OnceCell, time::sleep};

type CacheEntry = Arc<OnceCell<FetchResult>>;
pub struct LruProblemFetcher {
    cache: scc::HashCache<i64, CacheEntry>,
}

impl LruProblemFetcher {
    pub fn new() -> Self {
        Self {
            cache: scc::HashCache::new(),
        }
    }
}
#[async_trait]
impl ProblemFetcher for LruProblemFetcher {
    // it works only for Arc<Self>
    fn preload(self: &Arc<Self>, problem_id: i64) {
        // Preload all testcases for the given problem
        // This is a placeholder implementation
        let self_ = self.clone();
        tokio::spawn(async move {
            let _ = self_.get(problem_id).await;
        });
    }
    async fn get(self: &Arc<Self>, problem_id: i64) -> FetchResult {
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
                let result = Ok(Some(Arc::new(Problem {
                    id: problem_id,
                    memory_limit: 1024 * 1024 * 128,  // 128 MB
                    instruction_limit: 1_000_000_000, // 1 billion instructions
                    tests: vec![
                        Testcase {
                            input: "1 2".into(),
                            answer: "3".into(),
                        },
                        Testcase {
                            input: "4 5".into(),
                            answer: "9".into(),
                        },
                    ],
                })));
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
async fn test_lru_problem_fetcher() {
    let fetcher = Arc::new(LruProblemFetcher::new());

    // Preload a problem
    fetcher.preload(1);

    // Fetch the problem
    let result = fetcher.get(1).await;
    assert!(result.is_ok());
    let problem = result.unwrap();
    assert!(problem.is_some());
    let problem = problem.unwrap();
    assert_eq!(problem.id, 1);
    assert_eq!(problem.tests.len(), 2);

    // Fetch the same problem again to test caching
    let result2 = fetcher.get(1).await;
    assert!(result2.is_ok());
    let problem2 = result2.unwrap();
    assert!(problem2.is_some());
    let problem2 = problem2.unwrap();
    assert_eq!(problem2.id, 1);
}
