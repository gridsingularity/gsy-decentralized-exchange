FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives (a path dependency that also
# provides ../offchain-primitives/metadata.scale, embedded by the subxt macro).
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the community client.
WORKDIR /var/www/gsy-community-client
COPY ./gsy-community-client/Cargo.lock ./gsy-community-client/Cargo.toml /var/www/gsy-community-client/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
# Full offchain-primitives is required: it carries metadata.scale embedded via the
# subxt macro (`node_connector/orders.rs`).
COPY ./offchain-primitives /var/www/offchain-primitives
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY ./gsy-community-client /var/www/gsy-community-client
COPY --from=chef /var/www/gsy-community-client/target /var/www/gsy-community-client/target

WORKDIR /var/www/gsy-community-client
RUN cargo build --release

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssl && rm -rf /var/lib/apt/lists/*
WORKDIR /var/www/gsy-community-client
COPY --from=builder /var/www/gsy-community-client/target/release/gsy-community-client /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-community-client"]
