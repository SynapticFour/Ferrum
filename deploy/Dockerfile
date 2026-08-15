# Multi-stage: cargo-chef cache, release build (musl for static), distroless final (<20MB).
# Use for production/CI. For local demo use Dockerfile.gateway (debian-slim).
# Build: docker build -f deploy/Dockerfile -t ferrum-gateway:static .

# Stage 1: cargo-chef for dependency caching
FROM rust:1.91.1-bookworm AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef
COPY Cargo.toml Cargo.lock* ./
COPY crates ./crates
COPY tools ./tools
COPY third_party ./third_party
COPY deploy/scripts/clone-ga4gh-infra.sh /tmp/clone-ga4gh-infra.sh
ARG GA4GH_INFRA_REF=e43bf08bce5132db697a28c7db898d5e18f87b3f
RUN chmod +x /tmp/clone-ga4gh-infra.sh && GA4GH_INFRA_REF="${GA4GH_INFRA_REF}" /tmp/clone-ga4gh-infra.sh /ga4gh-infra
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: build release with musl for static binary
FROM rust:1.91.1-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools libssl-dev pkg-config git \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-musl
COPY --from=chef /usr/local/cargo/bin/cargo-chef /usr/local/cargo/bin/cargo-chef

COPY --from=chef /app/recipe.json recipe.json
COPY --from=chef /app/third_party ./third_party
COPY deploy/scripts/clone-ga4gh-infra.sh /tmp/clone-ga4gh-infra.sh
ARG GA4GH_INFRA_REF=e43bf08bce5132db697a28c7db898d5e18f87b3f
RUN chmod +x /tmp/clone-ga4gh-infra.sh && GA4GH_INFRA_REF="${GA4GH_INFRA_REF}" /tmp/clone-ga4gh-infra.sh /ga4gh-infra
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
