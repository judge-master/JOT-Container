pub mod aplusb_executor;
pub mod wasm_executor;

use async_trait::async_trait;

#[derive(Debug)]
pub enum ExecuteError {
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(usize),
    RuntimeError {
        reason: Option<String>,
    },
    CompileError {
        message: Option<String>,
    }
}
#[derive(Debug)]
pub struct ExecuteResult {
    pub output: String,
    pub memory_used: usize,
    pub instruction_count: u64,
}

#[async_trait]
pub trait Executor: Send {
    async fn execute(&mut self, input: &str, memory_limit: usize, time_limit: u64) -> Result<ExecuteResult, ExecuteError>;
}