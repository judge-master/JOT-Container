use async_trait::async_trait;
use super::Compiler;
use super::compile_executor::{CompileExecutor, simple_compile_executor::SimpleCompileExecutor};
use crate::grader::compiler::CompilerResult;
use crate::grader::executor::{wasm_executor::WasmExecutor};

pub struct CCompiler;
#[async_trait]
impl Compiler for CCompiler {
    async fn compile(&mut self, source: &str) -> Result<CompilerResult, String> {
        let args = ["-o", "-", "-std=c11", "-O2", "-Wall", "-x", "c", "-"];
        let additional_flags = std::env::var("CLANG_ADDITIONAL_FLAGS").unwrap_or("".into());
        let additional_flags = additional_flags
            .split_whitespace()
            .filter(|s| !s.is_empty());

        let mut compile_executor = SimpleCompileExecutor::new(
            std::env::var("CLANG_PATH").unwrap_or("clang".into()),
            args.into_iter().chain(additional_flags)
        );

        match compile_executor.execute(source, 0, 0).await {
            Ok(result) => {
                if !result.output.is_empty() {
                    let executor = Box::new(WasmExecutor::new(result.output));
                    Ok(CompilerResult { executor, diagnostics: result.diagnostics })
                } else {
                    Err(if result.diagnostics.is_empty() { "Compilation produced no Wasm output".into() } else { result.diagnostics })
                }
            },
            Err(e) => Err(format!("Compilation failed: {:?}", e)),
        }
    }
}

#[tokio::test]
async fn test_c_compiler() {
    dotenvy::dotenv().ok();

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
    match compiler.compile(source).await {
        Ok(compiler_result) => {
            println!("Compilation succeeded. Diagnostics: {}", compiler_result.diagnostics);
        },
        Err(err) => {
            println!("Compilation failed with error: {}", err);
            panic!("Compilation failed");
        }
    }
}
