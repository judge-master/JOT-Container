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

#[tokio::test]
async fn test_c_compiler() {
    let mut compiler = CCompiler;
    let source = r#"
    #include <stdio.h>
    int main() {
        int a, b;
        scanf("%d %d", &a, &b);
        printf("%d\n", a + b);
        return 0;
    }
    "#;

    let limits = CompilerResourceLimits {
        memory_limit_byte: 64 * 1024 * 1024,              // 64 MB
        time_limit_ms: 10000,                             // 10 seconds
        build_artifact_size_limit_byte: 10 * 1024 * 1024, // 10 MB
    };
    match compiler.compile(source, limits).await {
        Ok(compiler_result) => {
            println!(
                "Compilation succeeded. Diagnostics: {}",
                compiler_result.diagnostics
            );
        }
        Err(err) => {
            println!("Compilation failed with error: {:?}", err);
            panic!("Compilation failed");
        }
    }
}
