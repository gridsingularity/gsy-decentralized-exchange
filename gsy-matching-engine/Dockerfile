FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin gsy-matching-engine

FROM rust:1.95-bookworm AS runtime
WORKDIR /var/www/gsy-matching-engine
COPY --from=builder /var/www/target/release/gsy-matching-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-matching-engine", "web3", "http://gsy-offchain-storage", "8080", "ws://anvil", "8545"]
