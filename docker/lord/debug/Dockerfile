FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
COPY . .
RUN cargo build

FROM debian:bookworm-slim
LABEL org.opencontainers.image.source=https://github.com/SE-SS25/matrix_worker
RUN apt update
RUN apt install curl -y
COPY --from=builder /app/target/debug/matrix_worker /usr/local/bin/matrix_worker
HEALTHCHECK CMD curl -f "http://localhost:${PORT:-8080}/version" || exit 1
CMD ["matrix_worker"]
