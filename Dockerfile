# syntax=docker/dockerfile:1
ARG DEBIAN_VERSION=trixie-slim
ARG WASI_SDK_VERSION=34.0
ARG RUST_VERSION=1.96.1
ARG PROJECT_RUST=1.99.1
ARG GO_VERSION=1.27.3
ARG PYTHON_VERSION=3.14.7


# ---------------------------
# WASI SDK 설치
# --------------------------- 
# WASI SDK는 WASI용 C/C++ 컴파일러와 sysroot를 제공한다. WASI SDK는 WASI P1만 지원하므로, 다른 타깃의 sysroot는 제거한다.
FROM debian:${DEBIAN_VERSION} AS wasi-sdk-stage
ARG TARGETARCH
ARG WASI_SDK_VERSION
ARG WASI_SDK_ARCH

RUN apt-get update
RUN apt-get install -y --no-install-recommends wget ca-certificates

RUN --mount=type=cache,target=/downloads \
    set -eu; \
    sdk_arch=${WASI_SDK_ARCH:-${TARGETARCH}}; \
    case "$sdk_arch" in amd64) sdk_arch=x86_64 ;; esac; \
    sdk_archive=/downloads/wasi-sdk-${WASI_SDK_VERSION}-${sdk_arch}.tar.gz; \
    if [ ! -f "$sdk_archive" ]; then \
        wget --retry-connrefused --tries=3 "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-${WASI_SDK_VERSION%%.*}/wasi-sdk-${WASI_SDK_VERSION}-${sdk_arch}-linux.tar.gz" -O "$sdk_archive"; \
    fi; \
    mkdir -p /opt/wasi-sdk; \
    tar -xzf "$sdk_archive" -C /opt/wasi-sdk --strip-components=1; \
    # 채점기는 WASI P1만 사용하므로 다른 타깃의 sysroot는 배포하지 않는다.
    rm -rf /opt/wasi-sdk/share/wasi-sysroot/experimental-coop-threads \
        /opt/wasi-sdk/share/wasi-sysroot/include/wasm32-wasip1-threads \
        /opt/wasi-sdk/share/wasi-sysroot/include/wasm32-wasip2 \
        /opt/wasi-sdk/share/wasi-sysroot/include/wasm32-wasip3 \
        /opt/wasi-sdk/share/wasi-sysroot/lib/wasm32-wasip1-threads \
        /opt/wasi-sdk/share/wasi-sysroot/lib/wasm32-wasip2 \
        /opt/wasi-sdk/share/wasi-sysroot/lib/wasm32-wasip3; \
    /opt/wasi-sdk/bin/clang --version

# ENV 설정은 WASI SDK의 경로와 C/C++ 컴파일러를 지정한다. 또한, WASI용 C/C++ 컴파일 시 필요한 추가 플래그를 설정한다.
ENV WASI_SDK_PATH=/opt/wasi-sdk
ENV CLANG_PATH=/opt/wasi-sdk/bin/clang
ENV CLANGPP_PATH=/opt/wasi-sdk/bin/clang++
ENV CLANG_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/opt/wasi-sdk/share/wasi-sysroot"
ENV CLANGPP_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/opt/wasi-sdk/share/wasi-sysroot -fwasm-exceptions -mllvm -wasm-use-legacy-eh=false -lunwind"

# ---------------------------
# CPython의 WASI 타겟 빌드 
# --------------------------- 
FROM wasi-sdk-stage AS build-wasi-python-template
RUN apt-get install -y --no-install-recommends gcc libc6-dev make pkg-config xz-utils

ARG PYTHON_VERSION

RUN mkdir -p /src/cpython \
    && wget --tries=3 "https://www.python.org/ftp/python/${PYTHON_VERSION}/Python-${PYTHON_VERSION}.tar.xz" -O /tmp/python.tar.xz \
    && tar -xJf /tmp/python.tar.xz -C /src/cpython --strip-components=1 \
    && rm /tmp/python.tar.xz

# 빌드 보조 Python만 O1로 빠르게 만들고, 아래 WASI Python의 O3는 유지한다.
WORKDIR /build/native
RUN CFLAGS=-O1 /src/cpython/configure --without-ensurepip --disable-test-modules \
    && make -j "$(nproc)"

WORKDIR /build/wasi
RUN CC="/opt/wasi-sdk/bin/clang --target=wasm32-wasip1 --sysroot=/opt/wasi-sdk/share/wasi-sysroot" \
    AR=/opt/wasi-sdk/bin/llvm-ar \
    RANLIB=/opt/wasi-sdk/bin/llvm-ranlib \
    CONFIG_SITE=/src/cpython/Tools/wasm/wasi/config.site-wasm32-wasi \
    PKG_CONFIG_PATH= \
    PKG_CONFIG_LIBDIR=/opt/wasi-sdk/share/wasi-sysroot/lib/pkgconfig \
    PKG_CONFIG_SYSROOT_DIR=/opt/wasi-sdk/share/wasi-sysroot \
    /src/cpython/configure \
        --host=wasm32-wasip1 \
        --build="$(/src/cpython/config.guess)" \
        --with-build-python=/build/native/python \
        --without-ensurepip --disable-test-modules --disable-shared \
    && make -j "$(nproc)" libpython3.14.a python.wasm

COPY scripts/build_python_template.py /build/build_python_template.py
RUN /build/native/python /build/build_python_template.py \
    --source-dir /src/cpython --build-dir /build/wasi \
    --cc /opt/wasi-sdk/bin/clang --sysroot /opt/wasi-sdk/share/wasi-sysroot --output /out/python-template.wasm

# ---------------------------
# Rust 설치 도구만 준비한다. 인증서는 rustup의 HTTPS 다운로드에 사용한다.
# ---------------------------
FROM debian:${DEBIAN_VERSION} AS rustup-stage
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates rustup

# ---------------------------
# WASI용 Rust와 Go: SDK 및 Python 빌드와 독립적으로 준비한다.
# ---------------------------
FROM rustup-stage AS wasi-language-stage
ARG RUST_VERSION
RUN rustup toolchain install "${RUST_VERSION}" --profile minimal --target wasm32-wasip1 \
    && mv "$(rustup run "${RUST_VERSION}" rustc --print sysroot)" /opt/rust \
    && rm -rf "$(/opt/rust/bin/rustc --print target-libdir)" \
        /opt/rust/bin/cargo /opt/rust/bin/rustdoc \
        /opt/rust/bin/rust-gdb /opt/rust/bin/rust-gdbgui /opt/rust/bin/rust-lldb \
        /opt/rust/etc /opt/rust/libexec /opt/rust/share/man /opt/rust/share/zsh

RUN apt-get install -y --no-install-recommends wget
ARG GO_VERSION
ARG TARGETARCH
RUN wget --tries=3 "https://go.dev/dl/go${GO_VERSION}.linux-${TARGETARCH}.tar.gz" -O /tmp/go.tar.gz \
    && tar -C /opt -xzf /tmp/go.tar.gz \
    && rm /tmp/go.tar.gz \
    && rm -rf /opt/go/test

ENV PATH=/opt/rust/bin:/opt/go/bin:${PATH}
ENV GOROOT=/opt/go

# ---------------------------
# 프로젝트 빌드: 전체 Rust 도구 체인과 호스트 C 링커를 사용한다.
# ---------------------------
FROM rustup-stage AS project-rust-stage
RUN apt-get install -y --no-install-recommends gcc libc6-dev
ARG PROJECT_RUST
RUN rustup toolchain install "${PROJECT_RUST}" --profile minimal \
    && mv "$(rustup run "${PROJECT_RUST}" rustc --print sysroot)" /opt/project-rust
ENV PATH=/opt/project-rust/bin:${PATH}

FROM project-rust-stage AS chef
ENV CARGO_HOME=/usr/local/cargo
RUN cargo install cargo-chef --version 0.1.78 --locked --root /usr/local
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
ENV CARGO_PROFILE_RELEASE_STRIP=symbols
ENV GRPC_ADDR=0.0.0.0:50051
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --bin JOT-Container

# ---------------------------
# 실제 실행 환경: SDK, WASI용 언어 도구, Python 템플릿을 모은다.
# ---------------------------
FROM debian:${DEBIAN_VERSION} AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libc6 libgcc-s1 libstdc++6 libtinfo6 zlib1g \
    && rm -rf /var/lib/apt/lists/*

COPY --from=wasi-sdk-stage /opt/wasi-sdk /opt/wasi-sdk
COPY --from=wasi-language-stage /opt/rust /opt/rust
COPY --from=wasi-language-stage /opt/go /opt/go
COPY --from=build-wasi-python-template /out/python-template.wasm /opt/jot/python/python-template.wasm

ENV PATH=/opt/rust/bin:/opt/go/bin:${PATH}
ENV GRPC_ADDR=0.0.0.0:50051
ENV WASI_SDK_PATH=/opt/wasi-sdk
ENV CLANG_PATH=/opt/wasi-sdk/bin/clang
ENV CLANGPP_PATH=/opt/wasi-sdk/bin/clang++
ENV CLANG_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/opt/wasi-sdk/share/wasi-sysroot"
ENV CLANGPP_ADDITIONAL_FLAGS="--target=wasm32-wasip1 --sysroot=/opt/wasi-sdk/share/wasi-sysroot"
ENV RUSTC_PATH=/opt/rust/bin/rustc
ENV GOROOT=/opt/go
ENV GO_PATH=/opt/go/bin/go
ENV PYTHON_TEMPLATE_PATH=/opt/jot/python/python-template.wasm

RUN /opt/wasi-sdk/bin/clang --version \
    && rustc --version \
    && go version \
    && test -d /opt/rust/lib/rustlib/wasm32-wasip1/lib

# 테스트는 배포와 같은 실행 환경에 프로젝트용 Rust와 빌드 도구만 추가한다.
FROM runtime AS test
RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc libc6-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=project-rust-stage /opt/project-rust /opt/project-rust
COPY --from=chef /usr/local/bin/cargo-chef /usr/local/bin/cargo-chef
ENV PATH=/opt/project-rust/bin:${PATH}
ENV CARGO_HOME=/usr/local/cargo
ENV CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --locked --recipe-path recipe.json
COPY . .
RUN cargo test --locked

# 배포 이미지는 실행 환경에 서버 바이너리만 추가한다.
FROM runtime AS final
COPY --from=build /app/target/release/JOT-Container /usr/local/bin/JOT-Container
EXPOSE 50051
CMD ["JOT-Container"]
