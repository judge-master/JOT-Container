pub mod wasm_executor;

use async_trait::async_trait;

#[derive(Debug)]
pub enum ExecuteError {
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(u64),
    RuntimeError {
        reason: Option<String>,
    },
    CompilationError {
        message: Option<String>,
    }
}
#[derive(Debug)]
pub struct ExecuteResult {
    pub output: String,
    pub memory_used: u64,
    pub instruction_count: u64,
}

#[async_trait]
pub trait Executor: Send {
    async fn execute(&mut self, input: &str, memory_limit: u64, time_limit: u64) -> Result<ExecuteResult, ExecuteError>;
}

#[cfg(test)]
pub(crate) mod aplusb_executor;