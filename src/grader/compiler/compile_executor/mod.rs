pub mod simple_executor;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWriteExt};

#[derive(Debug)]
pub enum CompileExecuteError {
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(u64),
    RuntimeError {
        reason: Option<String>,
    }
}
#[derive(Debug)]
pub struct CompileExecuteResult {
    pub output: String,
}

#[async_trait]
pub trait CompileExecutor: Send {
    async fn execute(&mut self, input: &str, memory_limit: u64, time_limit: u64) -> Result<CompileExecuteResult, CompileExecuteError>;
}

#[async_trait]
pub trait StreamExecutor: Send {
    async fn execute(
        &mut self, 
        input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>, 
        memory_limit: u64, 
        time_limit: u64
    ) -> Result<CompileExecuteResult, CompileExecuteError>;
}

#[async_trait]
impl<T> CompileExecutor for T where T: StreamExecutor {
    async fn execute(&mut self, input: &str, memory_limit: u64, time_limit: u64) -> Result<CompileExecuteResult, CompileExecuteError> {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let input = String::from(input); // It has copy overhead, someday it has to be improved...
        let handle = tokio::spawn(async move {
            writer.write_all(input.as_bytes()).await.unwrap_or(());
            writer.shutdown().await.unwrap_or(());
        });

        let result = self.execute(Box::new(reader), memory_limit, time_limit).await;

        if let Err(_) = handle.await {
            return Err(CompileExecuteError::RuntimeError { reason: Some("Failed to write data to stream".into()) });
        }
        return result;
    }
}