FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin trade_executor

FROM rust:1.95-bookworm AS runtime

WORKDIR /usr/src/app
COPY --from=builder /var/www/target/release/trade_executor .
COPY --from=builder /var/www/e2e-tests/features ./features

ENV EVM_NODE_URL="ws://anvil:8545"
ENV OFFCHAIN_STORAGE_URL="http://gsy-offchain-storage:8080"
ENV RUST_LOG="info"

CMD ["./trade_executor"]
