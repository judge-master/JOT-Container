use std::path::{Path, PathBuf};
use async_trait::async_trait;
use super::{Executor, ExecuteError, ExecuteResult};
use wasmtime::{Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView, p1::{WasiP1Ctx, types::Error}, p2::pipe::{MemoryInputPipe, MemoryOutputPipe}};
use std::sync::Arc;

pub struct WasmExecutor {
    engine: Engine,
    path: PathBuf,
}

impl WasmExecutor {
    pub fn new(engine: Engine, path: &Path) -> Self {
        Self {
            engine,
            path: path.into()
        }
    }
}

struct MyState {
    ctx: WasiP1Ctx,
    limit: StoreLimits,
}
impl WasiView for MyState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        self.ctx.ctx()
    }
}

#[async_trait]
impl Executor for WasmExecutor {
    async fn execute(&mut self, input: &str, memory_limit: usize, time_limit: u64) -> Result<ExecuteResult, ExecuteError> {
        let engine = self.engine.clone();

        let limit = StoreLimitsBuilder::new()
                .memory_size(memory_limit)
                .memories(1)
                .instances(1)
                .build();

        let mut linker = Linker::<MyState>::new(&engine);
        if let Err(_) = wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s| &mut s.ctx) {
            return Err(ExecuteError::RuntimeError { reason: Some("add_to_linker_sync() failed".into()) });
        }

        let input = String::from(input);

        let memory_input = MemoryInputPipe::new(input);
        let memory_output = Arc::new(MemoryOutputPipe::new(1048576));
        let memory_error_output = Arc::new(MemoryOutputPipe::new(1048576));

        let wasi = WasiCtx::builder()
            //.inherit_stdout()
            .stdout(memory_output.clone())
            .stderr(memory_error_output.clone())
            .stdin(memory_input)
            .arg("StringExecutor")
            .build_p1();

        let mut store = Store::new(&engine, MyState {ctx: wasi, limit: limit});
        store.limiter(|store| &mut store.limit);
        let initial_fuel = time_limit;

        store.set_fuel(initial_fuel).map_err(|_| ExecuteError::RuntimeError { reason: Some("set_fuel() failed".into()) })?;

        let module = Module::from_file(&engine, &self.path)
            .map_err(|_| ExecuteError::RuntimeError { reason: Some("Failed to load wasm binary".into()) })?;
        linker.module(&mut store, "", &module)
            .map_err(|_| ExecuteError::RuntimeError { reason: Some("linker.module() failed".into()) })?;

        let instance = linker.instantiate(&mut store, &module).unwrap();

        let default = instance.get_func(&mut store, "_start")
            .ok_or(ExecuteError::RuntimeError { reason: Some("instance.get_func() failed".into()) })?;
        let default = default.typed::<(), ()>(&store)
            .map_err(|_| ExecuteError::RuntimeError { reason: Some("default.typed() failed".into()) })?;

        let (result, mut store) = 
            tokio::task::spawn_blocking(move || (default.call(&mut store, ()), store)).await
            .map_err(|_| ExecuteError::RuntimeError { reason: Some("tokio::task::spawn_blocking() failed".into()) })?;

        match result {
            Ok(()) => Ok(ExecuteResult {
                output: String::from_utf8_lossy(memory_output.contents().iter().as_slice()).into(),
                memory_used: instance.get_memory(&mut store, "memory")
                    .map(|m| m.size(&store) * m.page_size(&store)).unwrap_or(0) as usize,
                instruction_count: initial_fuel - store.get_fuel().unwrap()
            }),
            Err(e) => {
                if e.root_cause().downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
                    Err(ExecuteError::TimeLimitExceeded(time_limit))
                }
                // todo: MLE
                else {
                    Err(ExecuteError::RuntimeError { reason: Some(e.to_string()) })
                }
            }
        }
    }
}
