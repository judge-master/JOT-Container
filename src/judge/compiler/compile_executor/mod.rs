pub mod local;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWriteExt};

use crate::judge::compiler::{CompilerError, CompilerResourceLimits};

#[derive(Debug)]
pub struct CompileExecuteResult {
    pub output: Vec<u8>,
    pub diagnostics: String,
}

#[async_trait]
pub trait CompileExecutor: Send {
    async fn execute_stream(
        &mut self,
        input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>,
        limits: CompilerResourceLimits,
    ) -> Result<CompileExecuteResult, CompilerError>;

    async fn execute(
        &mut self,
        input: &str,
        limits: CompilerResourceLimits,
    ) -> Result<CompileExecuteResult, CompilerError> {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let input = String::from(input); // It has copy overhead, someday it has to be improved...
        let handle = tokio::spawn(async move {
            writer.write_all(input.as_bytes()).await.unwrap_or(());
            writer.shutdown().await.unwrap_or(());
        });

        let result = self.execute_stream(Box::new(reader), limits).await;

        if let Err(_) = handle.await {
            return Err(CompilerError::RuntimeError(
                "Failed to write data to stream".into(),
            ));
        }
        return result;
    }
}
