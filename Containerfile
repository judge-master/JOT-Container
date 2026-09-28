ARG RUST_VERSION=1.98.1
ARG ALPINE_VERSION=3.24.2
ARG COMPILER_CLANG_VERSION=22.1.3-r2
ARG COMPILER_RUST_VERSION=1.96.1-r0
ARG COMPILER_GO_VERSION=1.26.8-r0

FROM alpine:${ALPINE_VERSION} AS toolchain

ARG COMPILER_CLANG_VERSION
ARG COMPILER_RUST_VERSION
ARG COMPILER_GO_VERSION
RUN apk add --no-cache clang22=${COMPILER_CLANG_VERSION} lld22 wasi-libc wasi-libcxx wasi-compiler-rt \
    rust-wasm=${COMPILER_RUST_VERSION} go=${COMPILER_GO_VERSION}

FROM rust:${RUST_VERSION}-alpine AS build

WORKDIR /app
COPY Cargo.toml Cargo.lock ./

RUN mkdir src && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --locked

COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

FROM toolchain AS final
COPY --from=build /app/target/release/JOT-Container /usr/local/bin/JOT-Container

EXPOSE 50051
CMD ["JOT-Container"]
