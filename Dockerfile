FROM rustlang/rust:nightly-slim

# Needed to build some dependencies.
RUN apt-get update && apt-get install -y pkg-config libssl-dev


# Make a new project, copy over Cargo .toml & .lock and build to pull dependencies.
RUN USER=root cargo new --bin app
WORKDIR /app
COPY ./app/Cargo.toml /app/
COPY ./app/Cargo.lock /app/
RUN cargo build --release
RUN rm /app/src/*

# Copy real source & build real app.
COPY ./app/src /app/src

WORKDIR /app
RUN cargo install --path .

EXPOSE 2021

ENTRYPOINT ["app"]