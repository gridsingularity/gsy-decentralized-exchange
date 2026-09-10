FROM rustlang/rust:nightly-2026-08-20 AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the market orchestrator
WORKDIR /var/www/gsy-market-orchestrator
COPY ./gsy-market-orchestrator/Cargo.lock ./gsy-market-orchestrator/Cargo.toml /var/www/gsy-market-orchestrator/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY ./offchain-primitives /var/www/offchain-primitives
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY ./gsy-market-orchestrator /var/www/gsy-market-orchestrator
COPY --from=chef /var/www/gsy-market-orchestrator/target /var/www/gsy-market-orchestrator/target

WORKDIR /var/www/gsy-market-orchestrator
RUN cargo build --release

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssl && rm -rf /var/lib/apt/lists/*
WORKDIR /var/www/gsy-market-orchestrator
COPY --from=builder /var/www/gsy-market-orchestrator/target/release/gsy-market-orchestrator /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-market-orchestrator"]
