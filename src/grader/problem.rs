use std::sync::Arc;

// Arc is used for efficient caching between multiple threads
pub struct Testcase {
    pub input: Arc<str>,
    pub answer: Arc<str>,
}

pub struct Problem {
    pub id: String,
    pub memory_limit: u64,
    pub instruction_limit: u64,
    pub tests: Vec<Testcase>,
}
