# Multi-stage: cargo-chef cache, release build (musl for static), distroless final (<20MB).
# Use for production/CI. For local demo use Dockerfile.gateway (debian-slim).
# Build: docker build -f deploy/Dockerfile -t ferrum-gateway:static .

# Stage 1: cargo-chef for dependency caching
FROM rust:1-bookworm AS chef
WORKDIR /app
RUN cargo install cargo-chef
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
COPY tools ./tools
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: build release with musl for static binary
FROM rust:1-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl

COPY --from=chef /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p ferrum-gateway --target x86_64-unknown-linux-musl

COPY . .
RUN cargo build --release -p ferrum-gateway --target x86_64-unknown-linux-musl

# Stage 3: distroless static (no shell, minimal surface)
FROM gcr.io/distroless/static-debian12:nonroot
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/ferrum-gateway /app/ferrum-gateway
ENV FERRUM_BIND=0.0.0.0:8080
EXPOSE 8080
CMD ["/app/ferrum-gateway"]
