pub mod wasm_executor;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWriteExt};

#[derive(Debug)]
pub enum ExecuteError {
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(u64),
    RuntimeError { reason: Option<String> },
    CompilationError { message: Option<String> },
}
#[derive(Debug)]
pub struct ExecuteResult {
    pub output: String,
    pub memory_used: u64,
    pub instruction_count: u64,
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(
        &self,
        input: &str,
        memory_limit: u64,
        time_limit: u64,
    ) -> Result<ExecuteResult, ExecuteError>;
}

#[async_trait]
impl<T> Executor for Box<T>
where
    T: Executor + ?Sized,
{
    async fn execute(
        &self,
        input: &str,
        memory_limit: u64,
        time_limit: u64,
    ) -> Result<ExecuteResult, ExecuteError> {
        (**self).execute(input, memory_limit, time_limit).await
    }
}

#[cfg(test)]
pub(crate) mod aplusb_executor;
