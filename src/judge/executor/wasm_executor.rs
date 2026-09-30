use super::{ExecuteError, ExecuteResult, Executor};
use async_trait::async_trait;
use lazy_static::lazy_static;
use std::sync::Arc;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::{
    WasiCtx, WasiCtxView, WasiView,
    p1::WasiP1Ctx,
    p2::pipe::{MemoryInputPipe, MemoryOutputPipe},
};

lazy_static! {
    static ref WASMTIME_ENGINE: Engine = Engine::new(
        Config::new()
            .max_wasm_stack(1048576)
            .wasm_threads(false)
            .consume_fuel(true)
            .wasm_exceptions(true)
    )
    .expect("Failed to initialize Wasmtime engine");
}

pub struct WasmExecutor {
    engine: Engine,
    wasm: Vec<u8>,
}

impl WasmExecutor {
    pub fn new(wasm: Vec<u8>) -> Self {
        Self {
            engine: WASMTIME_ENGINE.clone(),
            wasm,
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
    async fn execute(
        &mut self,
        input: &str,
        memory_limit: u64,
        time_limit: u64,
    ) -> Result<ExecuteResult, ExecuteError> {
        let engine = self.engine.clone();

        let limit = StoreLimitsBuilder::new()
            .memory_size(memory_limit as usize)
            .memories(1)
            .instances(1)
            .build();

        let mut linker = Linker::<MyState>::new(&engine);
        if let Err(_) = wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s| &mut s.ctx) {
            return Err(ExecuteError::RuntimeError {
                reason: Some("add_to_linker_sync() failed".into()),
            });
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

        let mut store = Store::new(
            &engine,
            MyState {
                ctx: wasi,
                limit: limit,
            },
        );
        store.limiter(|store| &mut store.limit);
        let initial_fuel = time_limit;

        store
            .set_fuel(initial_fuel)
            .map_err(|_| ExecuteError::RuntimeError {
                reason: Some("set_fuel() failed".into()),
            })?;

        let module = Module::new(&engine, &self.wasm).map_err(|_| ExecuteError::RuntimeError {
            reason: Some("Failed to load wasm binary".into()),
        })?;
        linker
            .module(&mut store, "", &module)
            .map_err(|_| ExecuteError::RuntimeError {
                reason: Some("linker.module() failed".into()),
            })?;

        let instance = linker.instantiate(&mut store, &module).unwrap();

        let default =
            instance
                .get_func(&mut store, "_start")
                .ok_or(ExecuteError::RuntimeError {
                    reason: Some("instance.get_func() failed".into()),
                })?;
        let default = default
            .typed::<(), ()>(&store)
            .map_err(|_| ExecuteError::RuntimeError {
                reason: Some("default.typed() failed".into()),
            })?;

        let (result, mut store) =
            tokio::task::spawn_blocking(move || (default.call(&mut store, ()), store))
                .await
                .map_err(|_| ExecuteError::RuntimeError {
                    reason: Some("tokio::task::spawn_blocking() failed".into()),
                })?;

        match result {
            Ok(()) => Ok(ExecuteResult {
                output: String::from_utf8_lossy(memory_output.contents().iter().as_slice()).into(),
                memory_used: instance
                    .get_memory(&mut store, "memory")
                    .map(|m| m.size(&store) * m.page_size(&store))
                    .unwrap_or(0),
                instruction_count: initial_fuel - store.get_fuel().unwrap(),
            }),
            Err(e) => {
                if e.root_cause().downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) {
                    Err(ExecuteError::TimeLimitExceeded(time_limit))
                }
                // todo: MLE
                else {
                    Err(ExecuteError::RuntimeError {
                        reason: Some(e.to_string()),
                    })
                }
            }
        }
    }
}

#[tokio::test]
async fn test_wasm() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/aplusb.wasm")).to_vec(),
    );
    let res = executor
        .execute("1 2", 1048576, 100000)
        .await
        .expect("Runtime error");
    assert_eq!(res.output.trim(), "3");
}
#[tokio::test]
#[should_panic]
async fn test_wasm_tle() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/aplusb.wasm")).to_vec(),
    );
    let res = executor
        .execute("1 2", 1048576, 1000)
        .await
        .expect("Runtime error");
    assert_eq!(res.output.trim(), "3");
}
#[tokio::test]
#[should_panic]
async fn test_wasm_mle() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/aplusb.wasm")).to_vec(),
    );
    let res = executor
        .execute("1 2", 1000, 100000)
        .await
        .expect("Runtime error");
    assert_eq!(res.output.trim(), "3");
}
