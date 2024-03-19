ARG RUST_VERSION=1.76
ARG ALPINE_VERSION=3.19

FROM rust:${RUST_VERSION}-alpine${ALPINE_VERSION} AS chef
RUN apk add --no-cache musl-dev openssl-dev
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json .
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM alpine:${ALPINE_VERSION} AS runtime
RUN apk add --no-cache libgcc
COPY --from=builder /app/target/release/app .
ENTRYPOINT [ "/app" ]
