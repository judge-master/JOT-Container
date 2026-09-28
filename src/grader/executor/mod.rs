pub mod wasm_executor;
pub mod simple_executor;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWriteExt};

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

#[async_trait]
pub trait StreamExecutor: Send {
    async fn execute(
        &mut self, 
        input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>, 
        memory_limit: u64, 
        time_limit: u64
    ) -> Result<ExecuteResult, ExecuteError>;
}

#[async_trait]
impl Executor for dyn StreamExecutor {
    async fn execute(&mut self, input: &str, memory_limit: u64, time_limit: u64) -> Result<ExecuteResult, ExecuteError> {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let input = String::from(input); // It has copy overhead, someday it has to be improved...
        let handle = tokio::spawn(async move {
            writer.write_all(input.as_bytes()).await.unwrap_or(());
            writer.shutdown().await.unwrap_or(());
        });

        let result = self.execute(Box::new(reader), memory_limit, time_limit).await;

        if let Err(_) = handle.await {
            return Err(ExecuteError::RuntimeError { reason: Some("Failed to write data to stream".into()) });
        }
        return result;
    }
}

#[cfg(test)]
pub(crate) mod aplusb_executor;