FROM rust:1.84.1-bookworm AS builder

RUN rustup update nightly && rustup default nightly

WORKDIR /usr/src/app

ADD e2e-tests ./e2e-tests
ADD offchain-primitives ./offchain-primitives
ADD gsy-community-client ./gsy-community-client

WORKDIR /usr/src/app/e2e-tests

RUN cargo build --release --bin trade_executor

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/e2e-tests/target/release/trade_executor .
COPY --from=builder /usr/src/app/e2e-tests/features ./features

ENV GSY_NODE_URL="ws://gsy-node:9944"
ENV ORDERBOOK_SERVICE_URL="http://gsy-orderbook:8080"
ENV RUST_LOG="info"

CMD ["./trade_executor"]
