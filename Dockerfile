FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
RUN cargo install --locked trunk wasm-bindgen-cli
RUN rustup target add wasm32-unknown-unknown
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cd frontend && trunk build --release
RUN cargo build --release --bin server

ARG S3_BUCKET
ARG AWS_REGION=us-east-1
ARG AWS_ACCESS_KEY_ID
ARG AWS_SECRET_ACCESS_KEY

RUN cargo run --release --bin snakegpt-cli -- download --project battlesnake-community-docs

# We do not need the Rust toolchain to run the binary!
FROM debian:buster-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/server /usr/local/bin
COPY --from=builder /app/dist /app/dist
COPY --from=builder /app/*.db /app/

ENTRYPOINT ["/usr/local/bin/server"]
