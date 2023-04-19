FROM rust:latest as rust
WORKDIR /app

FROM rust AS builder 
RUN wget -qO- https://github.com/thedodd/trunk/releases/download/v0.16.0/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
RUN rustup target add wasm32-unknown-unknown

ARG APP_URL

COPY . .
RUN cd frontend && ../trunk build --release
RUN cargo build --release --bin server

ARG S3_BUCKET
ARG AWS_REGION=us-east-1
ARG AWS_ACCESS_KEY_ID
ARG AWS_SECRET_ACCESS_KEY

RUN cargo run --release --bin snakegpt-cli -- download --project battlesnake-community-docs

RUN mkdir -p /app/vendor && apt-get update && apt-get install -y curl libgomp1 libatlas-base-dev liblapack-dev \
  && curl -L -o vector0.tar.gz https://github.com/asg017/sqlite-vss/releases/download/v0.0.3/sqlite-vss-v0.0.3-vector0-linux-x86_64.tar.gz \
  && tar -xvzf vector0.tar.gz -C /app/vendor \
  && curl -L -o vss0.tar.gz https://github.com/asg017/sqlite-vss/releases/download/v0.0.3/sqlite-vss-v0.0.3-vss0-linux-x86_64.tar.gz \
  && tar -xvzf vss0.tar.gz -C /app/vendor

RUN curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 \
  && chmod +x tailwindcss-linux-x64 \
  && mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss

ENTRYPOINT ["cargo", "run", "--release", "--bin", "server"]
