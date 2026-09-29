use async_trait::async_trait;
use super::Compiler;
use std::path::PathBuf;
use super::compile_executor::{CompileExecutor, simple_executor::SimpleExecutor};
use crate::grader::executor::{Executor, wasm_executor::WasmExecutor};

pub struct CppCompiler;
#[async_trait]
impl Compiler for CppCompiler {
    async fn compile(&mut self, source: &str) -> Result<(Box<dyn Executor>, String), String> {
        let binary_path = self.create_binary_path();
        let args = ["-o", binary_path.to_str().unwrap_or(""), "-std=c++17", "-O2", "-Wall", "-x", "c++", "-"];
        let additional_flags = std::env::var("CLANGPP_ADDITIONAL_FLAGS").unwrap_or("".into());
        let additional_flags = additional_flags
            .split_whitespace()
            .filter(|s| !s.is_empty());

        let mut executor = SimpleExecutor::new(
            std::env::var("CLANGPP_PATH").unwrap_or("clang++".into()),
            args.into_iter().chain(additional_flags)
        );
        let result = executor.execute(source, 0, 0).await.map_err(|e| format!("Compilation failed: {:?}", e))?;
        if binary_path.try_exists().unwrap_or(false) {
            let executor = Box::new(WasmExecutor::new(&binary_path));
            Ok((executor, result.output))
        }
        else {
            Err(result.output)
        }
    }
}

#[tokio::test]
#[ignore = "Requires Clang++"]
async fn test_cpp_compiler() {
    dotenvy::dotenv().ok();

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
    match compiler.compile(source).await {
        Ok((executor, output)) => {
            println!("Compilation succeeded. Output: {}", output);
        },
        Err(err) => {
            println!("Compilation failed with error: {}", err);
            panic!("Compilation failed");
        }
    }
}