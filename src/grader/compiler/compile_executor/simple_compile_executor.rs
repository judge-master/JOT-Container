// SimpleCompileExecutor: a simple executor which execute a command without any security sandbox
// It can be used for implementing compile executors

use async_trait::async_trait;
use super::{StreamCompileExecutor, CompileExecuteError, CompileExecuteResult};
use tokio::io::{AsyncRead, AsyncWriteExt};
use tokio::process::Command;

pub struct SimpleCompileExecutor {
    command: String,
    args: Vec<String>,
}

impl SimpleCompileExecutor {
    pub fn new<S, I, S2>(command: S, args: I) -> Self
    where S: Into<String>, I: IntoIterator<Item = S2>, S2: Into<String> {
        Self {
            command: command.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl StreamCompileExecutor for SimpleCompileExecutor {
    // todo: implement memory limit and time limit
    async fn execute(
        &mut self, 
        mut input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>, 
        memory_limit: u64, 
        time_limit: u64
    ) -> Result<CompileExecuteResult, CompileExecuteError> {
        let Ok(mut child) = Command::new(&self.command)
            .args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn() else {
                return Err(CompileExecuteError::RuntimeError { reason: Some("Failed to execute".into()) });
            };

        let mut stdin = child.stdin.take().unwrap();

        let input_task = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut input, &mut stdin).await;
            let _ = stdin.shutdown().await;
        });

        let output = child.wait_with_output().await
            .map_err(|e| CompileExecuteError::RuntimeError { reason: Some(format!("Failed to read compiler output: {e}")) })?;
        input_task.await
            .map_err(|e| CompileExecuteError::RuntimeError { reason: Some(format!("Failed to write compiler input: {e}")) })?;

        Ok(CompileExecuteResult {
            output: output.stdout,
            diagnostics: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[tokio::test]
async fn captures_stdout_as_bytes_and_stderr_as_diagnostics() {
    use super::CompileExecutor;

    let mut executor = SimpleCompileExecutor::new("sh", ["-c", "cat; printf '\\377'; printf 'warning' >&2"]);
    let result = CompileExecutor::execute(&mut executor, "wasm", 0, 0).await.unwrap();

    assert_eq!(result.output, b"wasm\xff");
    assert_eq!(result.diagnostics, "warning");
}
