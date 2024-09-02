FROM docker.io/rust:1-slim-bookworm
RUN apt update && apt install -y git clang mold pkg-config libssl-dev
RUN cargo install cargo-watch
