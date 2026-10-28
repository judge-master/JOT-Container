use super::{ExecuteError, ExecuteResult, Executor};
use async_trait::async_trait;
use lazy_static::lazy_static;
use std::sync::Arc;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Trap};
use wasmtime_wasi::{HostMonotonicClock, HostWallClock, WasiCtxBuilder};
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

// this clock always returns same value to prevent non-deterministic codes depends on system clock
// unfortunately this custom clock doesn't prevent sleep()
// but it can prevent something like non-deterministic hashing based on clock
struct DeniedClock;
impl HostWallClock for DeniedClock {
    fn now(&self) -> std::time::Duration {
        std::time::Duration::from_secs(0)
    }
    fn resolution(&self) -> std::time::Duration {
        std::time::Duration::from_nanos(1)
    }
}
struct DeniedMonotonicClock;
impl HostMonotonicClock for DeniedMonotonicClock {
    fn now(&self) -> u64 {
        0
    }
    fn resolution(&self) -> u64 {
        1
    }
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
        &self,
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

        // shadow implementation of `poll_oneoff` to preventing sleeping
        // it immediately traps the instance if it tries to call poll_oneoff
        // it can prevent something like sleep() on <unistd.h>
        // but it may have side effects about input/output
        // currently our input stream immediately returns some value or EOF, so this works fine
        linker.allow_shadowing(true);

        linker
            .func_wrap(
                "wasi_snapshot_preview1",
                "poll_oneoff",
                |_in_ptr: i32,
                 _out_ptr: i32,
                 _nsubscriptions: i32,
                 _nevents_ptr: i32|
                 -> wasmtime::Result<i32> {
                    Err(wasmtime::Error::msg("poll_oneoff is disabled"))
                },
            )
            .map_err(|_| ExecuteError::RuntimeError {
                reason: Some("linker.func_wrap() failed".into()),
            })?;

        let input = String::from(input);

        let memory_input = MemoryInputPipe::new(input);
        let memory_output = Arc::new(MemoryOutputPipe::new(1048576));
        let memory_error_output = Arc::new(MemoryOutputPipe::new(1048576));

        let wasi = WasiCtx::builder()
            //.inherit_stdout()
            .stdout(memory_output.clone())
            .stderr(memory_error_output.clone())
            .stdin(memory_input)
            .wall_clock(DeniedClock)
            .monotonic_clock(DeniedMonotonicClock)
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

        let instance =
            linker
                .instantiate(&mut store, &module)
                .map_err(|_| ExecuteError::RuntimeError {
                    reason: Some("linker.instantiate() failed".into()),
                })?;

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
        .execute("1 2", 1048576, 10)
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

// poll_oneoff 를 disable 했기 때문에 이 fixture는 런타임 에러가 나야 함
#[tokio::test]
#[should_panic = "Runtime error"]
async fn test_wasm_sleep() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sleep.wasm")).to_vec(),
    );
    executor
        .execute("", 100000000, 100000000)
        .await
        .expect("Runtime error");
}

// WASI의 wall clock과 monotonic clock이 항상 0을 반환하도록 구현했기 때문에, time() 함수는 항상 0을 반환해야 함
#[tokio::test]
async fn test_wasm_time() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/time.wasm")).to_vec(),
    );
    let result = executor
        .execute("", 100000000, 100000000)
        .await
        .expect("Runtime error");

    assert_eq!(result.output.trim(), "0");
}

#[cfg(test)]
fn generate_long_input() -> String {
    let n = 100_000;
    let mut input = format!("{}\n", n);
    for i in 0..n {
        input.push_str(&format!("{} ", i));
    }
    input
}

#[tokio::test]
async fn test_long_input_scanf() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/long_input_scanf_strip.wasm"
        ))
        .to_vec(),
    );
    let input = generate_long_input();
    let result = executor
        .execute(&input, 100000000, 1000000000)
        .await
        .expect("Runtime error");

    assert_eq!(result.output.trim(), input.trim());
}

#[tokio::test]
async fn test_long_input_cin() {
    let mut executor = WasmExecutor::new(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/long_input_cin_strip.wasm"
        ))
        .to_vec(),
    );
    let input = generate_long_input();
    let result = executor
        .execute(&input, 100000000, 1000000000)
        .await
        .expect("Runtime error");

    assert_eq!(result.output.trim(), input.trim());
}
