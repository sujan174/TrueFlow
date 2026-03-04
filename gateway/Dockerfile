# ── Build Stage ───────────────────────────────────────────────────────────────
FROM rust:1.88-bookworm AS builder

WORKDIR /usr/src/trueflow

# Install build deps (cached layer)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Cache dependencies — copy manifests only, build a dummy crate
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build deps with docker-friendly settings: disable LTO and use more codegen
# units so the linker doesn't OOM inside Docker's constrained VM.
# CARGO_PROFILE_RELEASE_LTO=off overrides Cargo.toml [profile.release] lto=true
# CARGO_PROFILE_RELEASE_CODEGEN_UNITS=8 avoids the single-unit memory spike
ENV CARGO_PROFILE_RELEASE_LTO=off \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=8
RUN cargo build --release 2>/dev/null ; exit 0
RUN rm -rf src

# Copy real source and rebuild (only the app crate recompiles, deps are cached)
COPY . .
ENV SQLX_OFFLINE=true
RUN touch src/main.rs && cargo build --release --bin trueflow

# ── Runtime Stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary + migrations from builder
COPY --from=builder /usr/src/trueflow/target/release/trueflow /app/trueflow
COPY --from=builder /usr/src/trueflow/migrations /app/migrations

# Non-root user
RUN useradd -m trueflow
USER trueflow

EXPOSE 8443

# Disable core dumps to prevent key leakage
ENV MALLOC_TRIM_THRESHOLD_=0
CMD ["sh", "-c", "ulimit -c 0 && exec ./trueflow serve"]
