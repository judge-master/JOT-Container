// SimpleExecutor: a simple executor which execute a command without any security sandbox
// It can be used for implementing other executors

use async_trait::async_trait;
use super::{StreamExecutor, ExecuteResult, ExecuteError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub struct SimpleExecutor {
    command: String,
}

impl SimpleExecutor {
    fn new<T>(command: T) -> Self
    where String: From<T> {
        Self {
            command: command.into(),
        }
    }
}

#[async_trait]
impl StreamExecutor for SimpleExecutor {
    // todo: implement memory limit and time limit
    async fn execute(&mut self, mut input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>, memory_limit: u64, time_limit: u64) -> Result<ExecuteResult, ExecuteError> {
        let Ok(mut child) = Command::new(&self.command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn() else {
                return Err(ExecuteError::RuntimeError { reason: Some("Failed to execute".into()) });
            };

        let mut stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();

        tokio::spawn(async move {
            tokio::io::copy(&mut input, &mut stdin).await.expect("Error while writing to stdin");
            stdin.shutdown().await.expect("Shutdown error");
        });

        let mut buf = Vec::new();
        if let Err(_) = stdout.read_to_end(&mut buf).await {
            return Err(ExecuteError::RuntimeError { reason: Some("Failed to read the output".into()) });
        }

        Ok(ExecuteResult {
            output: String::from_utf8_lossy(&buf).into(),
            memory_used: 0,
            instruction_count: 0,
        })
    }
}
