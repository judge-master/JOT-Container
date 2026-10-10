use std::{
    sync::{
        Arc,
        atomic::{AtomicIsize, Ordering},
    },
    time::Duration,
};

use tokio::{sync::OnceCell, time::sleep};

#[derive(Clone)]
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

type CacheResult = Result<Option<Arc<Problem>>, String>;
type CacheEntry = Arc<OnceCell<CacheResult>>;
pub struct ProblemCache {
    cache: scc::HashCache<i64, CacheEntry>,
}

impl ProblemCache {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            cache: scc::HashCache::new(),
        }
    }
    // it works only for Arc<Self>
    pub fn preload(self: Arc<Self>, problem_id: i64) {
        // Preload all testcases for the given problem
        // This is a placeholder implementation
        tokio::spawn(async move {
            let _ = self.get(problem_id).await;
        });
    }
    pub async fn get(self: Arc<Self>, problem_id: i64) -> CacheResult {
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
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                self.cache
                    .remove_if_async(&key, |v| Arc::ptr_eq(v, &cell))
                    .await;
            });
        }

        result
    }

    fn clean(&self) {
        todo!()
    }
}
