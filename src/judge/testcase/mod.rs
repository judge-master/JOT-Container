use std::{
    sync::{Arc, atomic::AtomicUsize},
    time::Duration,
};

use tokio::{sync::OnceCell, time::sleep};

// Arc is used for efficient caching between multiple threads
#[derive(Clone)]
pub struct Testcase {
    pub input: Arc<str>,
    pub answer: Arc<str>,
}

pub struct Problem {
    pub id: i64,
    pub memory_limit: u64,
    pub instruction_limit: u64,
    pub tests: Vec<Testcase>,
}

pub struct TestcaseCache {
    max_cache_size: usize,
    current_cache_size: AtomicUsize,
    cache: scc::HashMap<(i64, i32), OnceCell<Testcase>>,
}

impl TestcaseCache {
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            max_cache_size,
            current_cache_size: AtomicUsize::new(0),
            cache: scc::HashMap::new(),
        }
    }
    // it works only for Arc<Self>
    pub fn preload(self: Arc<Self>, problem_id: i64) {
        // Preload all testcases for the given problem
        // This is a placeholder implementation
        tokio::spawn(async move {
            for i in 0..10 {
                self.get_testcase(problem_id, i).await;
            }
        });
    }
    pub async fn get_testcase(&self, problem_id: i64, testcase_id: i32) -> Testcase {
        let key = (problem_id, testcase_id);
        let cell = self
            .cache
            .entry_async(key)
            .await
            .or_insert_with(|| OnceCell::new());
        cell.get_or_init(|| {
            async {
                sleep(Duration::from_millis(50)).await; // Simulate a delay for loading the testcase
                // Load the testcase from the database or filesystem
                // For now, we just return a dummy testcase
                Testcase {
                    input: Arc::from("dummy input"),
                    answer: Arc::from("dummy answer"),
                }
            }
        })
        .await
        .clone()
    }
    pub async fn get_problem(&self, problem_id: i64) -> Problem {
        // Load the problem from the database or filesystem
        let test_count = 10; // Assume there are 10 testcases for the problem
        let mut tests = Vec::with_capacity(test_count);
        for i in 0..test_count {
            let testcase = self.get_testcase(problem_id, i as i32).await;
            tests.push(testcase);
        }
        Problem {
            id: problem_id,
            memory_limit: 1024 * 1024 * 128, // 128 MB
            instruction_limit: 1_000_000,    // 1 million instructions
            tests,
        }
    }

    fn clean(&self) {
        todo!()
    }
}
