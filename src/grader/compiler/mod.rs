use std::{path::{PathBuf}, sync::atomic::{AtomicU64, Ordering}};
use async_trait::async_trait;

use crate::grader::executor::{Executor, simple_executor::SimpleExecutor};

static BINARY_ID: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub trait Compiler {
    fn create_binary_path(&self) -> PathBuf {
        let mut path = PathBuf::from("build");
        path.push(format!("{}.out", BINARY_ID.fetch_add(1, Ordering::Relaxed)));
        path
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
        let mut executor = SimpleExecutor::new("gcc", args);
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