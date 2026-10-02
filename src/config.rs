use std::{env, net::SocketAddr, sync::OnceLock};

#[derive(Debug)]
struct Config {
    grpc_address: SocketAddr,
    clang_path: String,
    clangpp_path: String,
    clang_additional_flags: Vec<String>,
    clangpp_additional_flags: Vec<String>,
}

impl Config {
    fn load_from_env() -> Result<Self, String> {
        let grpc_address = env::var("GRPC_ADDR")
            .map_err(|_| "GRPC_ADDR environment variable is not set".to_string())
            .and_then(|addr| {
                addr.parse::<SocketAddr>()
                    .map_err(|_| "GRPC_ADDR is not a valid socket address".to_string())
            })?;

        let clang_path = env::var("CLANG_PATH")
            .map_err(|_| "CLANG_PATH environment variable is not set".to_string())?;
        let clangpp_path = env::var("CLANGPP_PATH")
            .map_err(|_| "CLANGPP_PATH environment variable is not set".to_string())?;

        let clang_additional_flags = env::var("CLANG_ADDITIONAL_FLAGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let clangpp_additional_flags = env::var("CLANGPP_ADDITIONAL_FLAGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        Ok(Config {
            grpc_address,
            clang_path,
            clangpp_path,
            clang_additional_flags,
            clangpp_additional_flags,
        })
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

// used only prodction code, not test code
pub fn init_config() -> Result<(), String> {
    let config = Config::load_from_env()?;
    CONFIG
        .set(config)
        .map_err(|_| "Failed to set config".to_string())
}

#[cfg(not(test))]
fn with_config<R>(f: impl FnOnce(&Config) -> R) -> R {
    f(CONFIG
        .get()
        .expect("Call init_config() before using config"))
}

// 단위 테스트: getter를 호출할 때마다 다시 로드하고 검증
#[cfg(test)]
fn with_config<R>(f: impl FnOnce(&Config) -> R) -> R {
    let loaded = Config::load_from_env().expect("Not valid environment variables for testing");
    f(&loaded)
}

pub fn grpc_address() -> SocketAddr {
    with_config(|config| config.grpc_address)
}
pub fn clang_path() -> String {
    with_config(|config| config.clang_path.clone())
}
pub fn clangpp_path() -> String {
    with_config(|config| config.clangpp_path.clone())
}
pub fn clang_additional_flags() -> Vec<String> {
    with_config(|config| config.clang_additional_flags.clone())
}
pub fn clangpp_additional_flags() -> Vec<String> {
    with_config(|config| config.clangpp_additional_flags.clone())
}
