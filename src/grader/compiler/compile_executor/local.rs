// SimpleCompileExecutor: a simple executor which execute a command without any security sandbox
// It can be used for implementing compile executors


use async_trait::async_trait;
use tokio::time::timeout;
use super::{CompileExecutor, CompileExecuteError, CompileExecuteResult};
use crate::grader::compiler::CompilerResourceLimits;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

pub struct LocalCompileExecutor {
    command: String,
    args: Vec<String>,
}

impl LocalCompileExecutor {
    pub fn new<S, I, S2>(command: S, args: I) -> Self
    where S: Into<String>, I: IntoIterator<Item = S2>, S2: Into<String> {
        Self {
            command: command.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl CompileExecutor for LocalCompileExecutor {
    // todo: implement memory limit
    async fn execute_stream(
        &mut self, 
        mut input: Box<dyn AsyncRead + Send + Sync + Unpin + 'static>,
        limits: CompilerResourceLimits,
    ) -> Result<CompileExecuteResult, CompileExecuteError> {

        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| CompileExecuteError::RuntimeError("Failed to spawn process".into()))?;

        let mut stdin = child.stdin.take().ok_or(CompileExecuteError::RuntimeError("Failed to open stdin".into()))?;
        let mut stdout = child.stdout.take().ok_or(CompileExecuteError::RuntimeError("Failed to open stdout".into()))?;
        let mut stderr = child.stderr.take().ok_or(CompileExecuteError::RuntimeError("Failed to open stderr".into()))?;

        let input_handle = tokio::spawn(async move {
            tokio::io::copy(&mut input, &mut stdin).await.unwrap_or(0);
        });

        let mut stdout_handle = tokio::spawn(async move {
            let mut output = Vec::new();
            let mut output_chunk = [0u8; 8192];
            loop {
                let n = stdout.read(&mut output_chunk).await
                    .map_err(|_| CompileExecuteError::RuntimeError("Failed to read stdout".into()))?;
                if n == 0 {
                    break;
                }
                if output.len() + n > limits.build_artifact_size_limit_byte as usize {
                    return Err(CompileExecuteError::BuildArtifactSizeLimitExceeded(limits.build_artifact_size_limit_byte));
                }
                output.extend_from_slice(&output_chunk[..n]);
            }
            Ok::<Vec<u8>, CompileExecuteError>(output)
        });

        let mut stderr_handle = tokio::spawn(async move {
            let mut diagnostics = Vec::new();
            tokio::io::copy(&mut stderr, &mut diagnostics).await.unwrap_or(0);
            Ok::<Vec<u8>, CompileExecuteError>(diagnostics)
        });

        let mut stdout_joined = false;
        let mut stderr_joined = false;
        let mut child_reaped = false;
        let result = timeout(tokio::time::Duration::from_millis(limits.time_limit_ms), async {
            let stdout_result = (&mut stdout_handle).await;
            stdout_joined = true;
            let output = match stdout_result {
                Ok(Ok(output)) => output,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(CompileExecuteError::RuntimeError("stdout 작업 실패".into())),
            };

            let stderr_result = (&mut stderr_handle).await;
            stderr_joined = true;
            let diagnostics = match stderr_result {
                Ok(Ok(diagnostics)) => diagnostics,
                Ok(Err(err)) => return Err(err),
                Err(_) => return Err(CompileExecuteError::RuntimeError("stderr 작업 실패".into())),
            };

            let status = child.wait().await
                .map_err(|_| CompileExecuteError::RuntimeError("Failed to wait for process".into()))?;
            child_reaped = true;
            if !status.success() {
                return Err(CompileExecuteError::CompilationError(String::from_utf8_lossy(&diagnostics).to_string()));
            }

            Ok(CompileExecuteResult {
                output,
                diagnostics: String::from_utf8_lossy(&diagnostics).to_string(),
            })
        }).await.unwrap_or_else(|_| Err(CompileExecuteError::TimeLimitExceeded(limits.time_limit_ms)));

        if result.is_err() {
            stdout_handle.abort();
            stderr_handle.abort();
            if !child_reaped {
                let _ = child.start_kill();
                let _ = child.wait().await;
            }
            if !stdout_joined {
                let _ = stdout_handle.await;
            }
            if !stderr_joined {
                let _ = stderr_handle.await;
            }
        }

        input_handle.abort();
        let _ = input_handle.await;
        result
    }
}

#[tokio::test]
async fn captures_stdout_as_bytes_and_stderr_as_diagnostics() {
    use super::CompileExecutor;
    let limits = CompilerResourceLimits {
        memory_limit_byte: 64 * 1024 * 1024,
        time_limit_ms: 5000,
        build_artifact_size_limit_byte: 10 * 1024 * 1024,
    };

    let mut executor = LocalCompileExecutor::new("sh", ["-c", "cat; printf '\\377'; printf 'warning' >&2"]);
    let result = CompileExecutor::execute(&mut executor, "wasm", limits).await.unwrap();

    assert_eq!(result.output, b"wasm\xff");
    assert_eq!(result.diagnostics, "warning");
}

#[cfg(test)]
fn assert_process_stopped(pid_file: &std::path::Path) {
    let pid = std::fs::read_to_string(pid_file).unwrap();
    let pid = pid.trim();
    let alive = std::process::Command::new("sh")
        .args(["-c", &format!("kill -0 {pid} 2>/dev/null")])
        .status()
        .unwrap()
        .success();
    if alive {
        let _ = std::process::Command::new("sh")
            .args(["-c", &format!("kill -9 {pid} 2>/dev/null")])
            .status();
    }
    std::fs::remove_file(pid_file).unwrap();
    assert!(!alive, "compiler process {pid} survived the limit");
}

#[tokio::test]
async fn time_limit_stops_compiler_with_blocked_input() {
    use super::CompileExecutor;

    let pid_file = std::env::temp_dir().join(format!("jot-compiler-timeout-{}.pid", std::process::id()));
    let script = format!("echo $$ > '{}'; exec sleep 5", pid_file.display());
    let mut executor = LocalCompileExecutor::new("sh", ["-c", &script]);
    let limits = CompilerResourceLimits {
        memory_limit_byte: 64 * 1024 * 1024,
        time_limit_ms: 200,
        build_artifact_size_limit_byte: 1024,
    };
    let source = "x".repeat(1024 * 1024);
    let result = timeout(tokio::time::Duration::from_secs(2), executor.execute(&source, limits))
        .await
        .expect("execute did not return after the time limit");

    assert!(matches!(result, Err(CompileExecuteError::TimeLimitExceeded(200))));
    assert_process_stopped(&pid_file);
}

#[tokio::test]
async fn artifact_limit_stops_compiler() {
    use super::CompileExecutor;

    let pid_file = std::env::temp_dir().join(format!("jot-compiler-output-{}.pid", std::process::id()));
    let script = format!("echo $$ > '{}'; printf 'oversized'; exec sleep 5", pid_file.display());
    let mut executor = LocalCompileExecutor::new("sh", ["-c", &script]);
    let limits = CompilerResourceLimits {
        memory_limit_byte: 64 * 1024 * 1024,
        time_limit_ms: 2000,
        build_artifact_size_limit_byte: 4,
    };
    let result = timeout(tokio::time::Duration::from_secs(2), executor.execute("", limits))
        .await
        .expect("execute did not return after the artifact limit");

    assert!(matches!(result, Err(CompileExecuteError::BuildArtifactSizeLimitExceeded(4))));
    assert_process_stopped(&pid_file);
}
