FROM rust:latest as rust
WORKDIR /app

FROM rust AS builder 
RUN wget -qO- https://github.com/thedodd/trunk/releases/download/v0.16.0/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
RUN rustup target add wasm32-unknown-unknown

COPY . .
RUN cd frontend && ../trunk build --release
RUN cargo build --release --bin server

ARG S3_BUCKET
ARG AWS_REGION=us-east-1
ARG AWS_ACCESS_KEY_ID
ARG AWS_SECRET_ACCESS_KEY

RUN cargo run --release --bin snakegpt-cli -- download --project battlesnake-community-docs

ENTRYPOINT ["cargo", "run", "--release", "--bin", "server"]
