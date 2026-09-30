use super::Compiler;
use super::compile_executor::{CompileExecutor, local::LocalCompileExecutor};
use crate::judge::compiler::{CompilerResourceLimits, CompilerResult};
use crate::judge::executor::wasm_executor::WasmExecutor;
use async_trait::async_trait;

pub struct CppCompiler;
#[async_trait]
impl Compiler for CppCompiler {
    async fn compile(
        &mut self,
        source: &str,
        limits: CompilerResourceLimits,
    ) -> Result<(CompilerResult), String> {
        let args = ["-o", "-", "-std=c++17", "-O2", "-Wall", "-x", "c++", "-"];
        let additional_flags = std::env::var("CLANGPP_ADDITIONAL_FLAGS").unwrap_or("".into());
        let additional_flags = additional_flags
            .split_whitespace()
            .filter(|s| !s.is_empty());

        let mut compile_executor = LocalCompileExecutor::new(
            std::env::var("CLANGPP_PATH").unwrap_or("clang++".into()),
            args.into_iter().chain(additional_flags),
        );

        match compile_executor.execute(source, limits).await {
            Ok(result) => {
                if !result.output.is_empty() {
                    let executor = Box::new(WasmExecutor::new(result.output));
                    Ok(CompilerResult {
                        executor,
                        diagnostics: result.diagnostics,
                    })
                } else {
                    Err(if result.diagnostics.is_empty() {
                        "Compilation produced no Wasm output".into()
                    } else {
                        result.diagnostics
                    })
                }
            }
            Err(e) => Err(format!("Compilation failed: {:?}", e)),
        }
    }
}

#[tokio::test]
async fn test_cpp_compiler() {
    let mut compiler = CppCompiler;
    let source = r#"
    #include <iostream>
    int main() {
        int a, b;
        std::cin >> a >> b;
        std::cout << a + b << std::endl;
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
            println!("Compilation failed with error: {}", err);
            panic!("Compilation failed");
        }
    }
}
