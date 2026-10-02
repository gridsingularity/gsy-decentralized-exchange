FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin gsy-market-orchestrator

FROM rust:1.95-bookworm AS runtime
WORKDIR /var/www/gsy-market-orchestrator
COPY --from=builder /var/www/target/release/gsy-market-orchestrator /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-market-orchestrator"]
