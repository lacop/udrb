FROM rustlang/rust:nightly-slim

# Needed to build some dependencies.
RUN apt-get update && apt-get install -y pkg-config libssl-dev

RUN mkdir /app

# Copy Cargo.toml and .lock first
COPY ./app/Cargo.toml /app/
COPY ./app/Cargo.lock /app/

# Pull & build dependencies.
RUN cargo build --release

# Copy source.
COPY ./app/src /app/src

WORKDIR /app
RUN cargo install --path .

EXPOSE 2021

ENTRYPOINT ["app"]