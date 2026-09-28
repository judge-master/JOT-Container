use std::{path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};
use async_trait::async_trait;

use crate::grader::executor::{Executor, simple_executor::SimpleExecutor};

static BINARY_ID: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub trait Compiler {
    fn create_binary_path(&self) -> PathBuf {
        let build_dir = std::env::var("BUILD_DIR").expect("BUILD_DIR environment is needed");
        let build_dir = Path::new(&build_dir);

        build_dir.join(format!("{}.out", BINARY_ID.fetch_add(1, Ordering::Relaxed)))
    }

    // returns the path to the compiled binary if successful, or an error message if failed
    async fn compile(&mut self, source: &str) -> Result<(PathBuf, String), String>;
}

pub struct CCompiler;
#[async_trait]
impl Compiler for CCompiler {
    async fn compile(&mut self, source: &str) -> Result<(PathBuf, String), String> {
        let binary_path = self.create_binary_path();
        let args = ["-o", binary_path.to_str().unwrap_or(""), "-std=c11", "-O2", "-Wall", "-x", "c", "-"];
        let additional_flags = std::env::var("CLANG_ADDITIONAL_FLAGS").unwrap_or("".into());
        let additional_flags = additional_flags
            .split_whitespace()
            .filter(|s| !s.is_empty());

        let mut executor = SimpleExecutor::new(
            std::env::var("CLANG_PATH").unwrap_or("clang".into()),
            args.into_iter().chain(additional_flags)
        );
        let result = executor.execute(source, 0, 0).await.map_err(|e| format!("Compilation failed: {:?}", e))?;
        if binary_path.try_exists().unwrap_or(false) {
            Ok((binary_path, result.output))
        }
        else {
            Err(result.output)
        }
    }
}

pub struct CppCompiler;
#[async_trait]
impl Compiler for CppCompiler {
    async fn compile(&mut self, source: &str) -> Result<(PathBuf, String), String> {
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
            Ok((binary_path, result.output))
        }
        else {
            Err(result.output)
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
        Ok((binary_path, output)) => {
            println!("Compilation succeeded. Binary path: {:?}, Output: {}", binary_path, output);
            assert!(binary_path.try_exists().unwrap_or(false));
        },
        Err(err) => {
            println!("Compilation failed with error: {}", err);
            panic!("Compilation failed");
        }
    }
}

#[tokio::test]
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
        Ok((binary_path, output)) => {
            println!("Compilation succeeded. Binary path: {:?}, Output: {}", binary_path, output);
            assert!(binary_path.try_exists().unwrap_or(false));
        },
        Err(err) => {
            println!("Compilation failed with error: {}", err);
            panic!("Compilation failed");
        }
    }
}