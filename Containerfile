# for building the JOT-Container
ARG RUST_VERSION=1.98.1

# for building the final image
ARG ALPINE_VERSION=3.24.2
ARG COMPILER_GPP_VERSION=15.2.0-r5
ARG COMPILER_RUST_VERSION=1.96.1-r0
ARG COMPILER_GO_VERSION=1.26.8-r0



FROM rust:${RUST_VERSION}-alpine AS build

WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./

RUN mkdir src && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release --locked

COPY src ./src
RUN cargo build --release --locked

FROM alpine:${ALPINE_VERSION} AS final

ARG COMPILER_GPP_VERSION
ARG COMPILER_RUST_VERSION
ARG COMPILER_GO_VERSION

RUN apk add --no-cache g++=${COMPILER_GPP_VERSION}
RUN apk add --no-cache rust=${COMPILER_RUST_VERSION}
RUN apk add --no-cache go=${COMPILER_GO_VERSION}


COPY --from=build /app/target/release/JOT-Container /usr/local/bin/JOT-Container

CMD ["JOT-Container"]
