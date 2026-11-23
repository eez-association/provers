# syntax=docker/dockerfile:1
#
# eez-prover-stateful image: the stateful ECDSA proof signer. Build with this
# directory as the context:
#
#   docker build -t eez-prover-stateful stateful-ecdsa

# ── chef base: toolchain + system deps ───────────────────────────────
FROM rust:1.98.1-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang libclang-dev pkg-config cmake libssl-dev git ca-certificates mold \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --locked
# mold shortens the final link. cook and build see the same RUSTFLAGS, so the
# cooked dependencies stay fresh for the source layer.
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"
WORKDIR /build

# ── planner: compute the dependency recipe from manifests ────────────
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

# ── builder: cook deps (cached), then build the signer ───────────────
FROM chef AS builder
# `release` (cgu=16, stripped, thin LTO) is the fast-to-build default;
# BUILD_PROFILE=maxperf for production binaries.
ARG BUILD_PROFILE=release
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo chef cook --locked --profile "$BUILD_PROFILE" --recipe-path recipe.json \
        --package eez-prover-stateful
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo build --locked --profile "$BUILD_PROFILE" -p eez-prover-stateful --bin eez-prover-stateful \
    && cp "target/$BUILD_PROFILE/eez-prover-stateful" /build/eez-prover-stateful

# ── runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS proof-signer
RUN apt-get update && apt-get install -y --no-install-recommends netcat-openbsd ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder --chmod=0555 /build/eez-prover-stateful /usr/local/bin/eez-prover-stateful
WORKDIR /app
EXPOSE 50061/tcp
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/local/bin/eez-prover-stateful"]

CMD ["--help"]
