use super::Compiler;
use super::compile_executor::{CompileExecutor, local::LocalCompileExecutor};
use crate::judge::compiler::{CompilerError, CompilerResourceLimits, CompilerResult};
use crate::judge::executor::wasm_executor::WasmExecutor;
use async_trait::async_trait;

pub struct CCompiler;
#[async_trait]
impl Compiler for CCompiler {
    async fn compile(
        &mut self,
        source: &str,
        limits: CompilerResourceLimits,
    ) -> Result<CompilerResult, CompilerError> {
        let args = ["-o", "-", "-std=c11", "-O2", "-Wall", "-x", "c", "-"];
        let additional_flags = std::env::var("CLANG_ADDITIONAL_FLAGS").unwrap_or("".into());
        let additional_flags = additional_flags
            .split_whitespace()
            .filter(|s| !s.is_empty());

        let mut compile_executor = LocalCompileExecutor::new(
            std::env::var("CLANG_PATH").unwrap_or("clang".into()),
            args.into_iter().chain(additional_flags),
        );

        let result = compile_executor.execute(source, limits).await?;
        Ok(CompilerResult {
            executor: Box::new(WasmExecutor::new(result.output)),
            diagnostics: result.diagnostics,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CCompiler;
    use crate::judge::compiler::test_support::{
        ExpectedError, assert_compile_error, assert_compiles, default_limits,
    };

    const SOURCE: &str = r#"
    #include <stdio.h>
    int main() {
        int a, b;
        scanf("%d %d", &a, &b);
        printf("%d\n", a + b);
        return 0;
    }
    "#;

    #[tokio::test]
    async fn test_c_compiler() {
        assert_compiles(&mut CCompiler, SOURCE).await;
    }

    #[tokio::test]
    async fn test_c_compiler_tle() {
        let mut limits = default_limits();
        limits.time_limit_ms = 0;
        assert_compile_error(
            &mut CCompiler,
            SOURCE,
            limits,
            ExpectedError::TimeLimitExceeded(0),
        )
        .await;
    }

    #[tokio::test]
    async fn test_c_compiler_artifact_size_le() {
        let mut limits = default_limits();
        limits.build_artifact_size_limit_byte = 0;
        assert_compile_error(
            &mut CCompiler,
            SOURCE,
            limits,
            ExpectedError::BuildArtifactSizeLimitExceeded(0),
        )
        .await;
    }
}
