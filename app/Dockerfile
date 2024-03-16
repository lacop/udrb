ARG RUST_VERSION=1.76
ARG ALPINE_VERSION=3.19

FROM rust:${RUST_VERSION}-alpine${ALPINE_VERSION} AS builder
RUN apk add --no-cache musl-dev openssl-dev
ENV RUSTFLAGS="-C target-feature=-crt-static"
WORKDIR /app
COPY ./ /app
RUN cargo build --release

FROM alpine:${ALPINE_VERSION}
RUN apk add --no-cache libgcc
COPY --from=builder /app/target/release/app .
ENTRYPOINT [ "/app" ]
