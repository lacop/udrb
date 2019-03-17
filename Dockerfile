FROM rustlang/rust:nightly-slim

# Needed to build some dependencies.
RUN apt-get update && apt-get install -y pkg-config libssl-dev

RUN mkdir /app

# Only copy src/ and Cargo.toml, not target/...
COPY ./app/Cargo.toml /app/
COPY ./app/src /app/src

WORKDIR /app
RUN cargo install --path .

EXPOSE 2021

ENTRYPOINT ["app"]