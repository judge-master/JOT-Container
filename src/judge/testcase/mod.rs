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
    max_cache_size: usize,
    current_cache_size: Arc<AtomicIsize>,
    cache: scc::HashCache<i64, CacheEntry>,
}

impl ProblemCache {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            max_cache_size,
            current_cache_size: Arc::new(AtomicIsize::new(0)),
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
    pub async fn get(&self, problem_id: i64) -> CacheResult {
        let key = problem_id;
        let current_cache_size = self.current_cache_size.clone();
        let cell = self
            .cache
            .entry_async(key)
            .await
            .or_put_with(|| Arc::new(OnceCell::new()))
            .1
            .clone();
        cell.get_or_init(|| async {
            sleep(Duration::from_millis(50)).await;
            // Load the testcase from the database or filesystem
            // For now, we just return a dummy problem
            let result = Ok(Some(Arc::new(Problem {
                id: problem_id,
                memory_limit: 0,
                instruction_limit: 0,
                tests: vec![
                    Testcase {
                        input: "dummy input".into(),
                        answer: "dummy answer".into(),
                    },
                    Testcase {
                        input: "dummy input 2".into(),
                        answer: "dummy answer 2".into(),
                    },
                ],
            })));
            // Update the current cache size
            if let Ok(Some(problem)) = &result {
                let now_size = problem
                    .tests
                    .iter()
                    .map(|t| t.input.len() + t.answer.len())
                    .sum::<usize>();
                current_cache_size.fetch_add(now_size as isize, Ordering::Relaxed);
            }
            result
        })
        .await
        .clone()
    }

    fn clean(&self) {
        todo!()
    }
}
