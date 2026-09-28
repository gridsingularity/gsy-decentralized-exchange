FROM rustlang/rust:nightly-2026-08-20 AS chef
# Pinned deliberately: the floating `nightly` tag moved to a build whose never-type
# fallback change breaks the pinned `mitsuhiko/redis-rs` revision the matching engine
# depends on, so an unpinned rebuild of the handoff kit fails partway through.
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the matching engine
WORKDIR /var/www/gsy-matching-engine
COPY ./gsy-matching-engine/Cargo.lock ./gsy-matching-engine/Cargo.toml /var/www/gsy-matching-engine/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY ./offchain-primitives /var/www/offchain-primitives
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY ./gsy-matching-engine /var/www/gsy-matching-engine
COPY --from=chef /var/www/gsy-matching-engine/target /var/www/gsy-matching-engine/target

WORKDIR /var/www/gsy-matching-engine
RUN cargo build --release

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssl && rm -rf /var/lib/apt/lists/*
WORKDIR /var/www/gsy-matching-engine
COPY --from=builder /var/www/gsy-matching-engine/target/release/gsy-matching-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-matching-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944"]
