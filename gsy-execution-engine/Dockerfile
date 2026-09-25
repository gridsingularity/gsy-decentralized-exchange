FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin gsy-execution-engine

FROM rust:1.95-bookworm AS runtime
WORKDIR /var/www/gsy-execution-engine
COPY --from=builder /var/www/target/release/gsy-execution-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-execution-engine", "web3", "http://gsy-offchain-storage", "8080", "ws://anvil", "8545", "30", "3600", "0.10"]
