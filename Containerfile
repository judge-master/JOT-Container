ARG ALPINE_VERSION=3.24.2
ARG COMPILER_CLANG_VERSION=22.1.3-r2
ARG COMMON_RUST_VERSION=1.96.1-r0
ARG COMPILER_GO_VERSION=1.26.8-r0
ARG CARGO_VERSION=1.98.1-r0

# 사용자 코드를 빌드하기 위한 도구 체인을 설치하는 단계
FROM alpine:${ALPINE_VERSION} AS toolchain

ARG COMPILER_CLANG_VERSION
ARG COMMON_RUST_VERSION
ARG COMPILER_GO_VERSION
RUN apk add --no-cache clang22=${COMPILER_CLANG_VERSION} lld22 wasi-libc wasi-libcxx wasi-compiler-rt \
    rust-wasm=${COMMON_RUST_VERSION} go=${COMPILER_GO_VERSION}

ENV GRPC_ADDR=0.0.0.0:50051

ENV CLANG_PATH=/usr/bin/clang
ENV CLANGPP_PATH=/usr/bin/clang++
ENV RUSTC_PATH=/usr/bin/rustc
ENV GO_PATH=/usr/bin/go

ENV CLANG_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/usr/share/wasi-sysroot"
ENV CLANGPP_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/usr/share/wasi-sysroot"

# CI에서 Rust 테스트를 실행하는 타겟
FROM toolchain AS test

RUN apk add --no-cache cargo=${COMMON_RUST_VERSION}

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests

RUN cargo test --locked

# 프로젝트의 Rust 코드를 빌드하는 단계
FROM alpine:${ALPINE_VERSION} AS build

ARG COMMON_RUST_VERSION

RUN apk add --no-cache cargo=${COMMON_RUST_VERSION}

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

RUN mkdir src && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --locked

COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

# 최종 이미지를 생성하는 단계
FROM toolchain AS final

COPY --from=build /app/target/release/JOT-Container /usr/local/bin/JOT-Container

EXPOSE 50051

CMD ["JOT-Container"]
