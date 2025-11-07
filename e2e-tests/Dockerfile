FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /app/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /app/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Then dependencies of the community client
WORKDIR /app/gsy-community-client
COPY ./gsy-community-client/Cargo.lock ./gsy-community-client/Cargo.toml /app/gsy-community-client/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Then dependencies of the community client
WORKDIR /app/e2e-tests
COPY ./e2e-tests/Cargo.lock ./e2e-tests/Cargo.toml /app/e2e-tests/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
COPY e2e-tests ./e2e-tests
COPY --from=chef /app/e2e-tests/target /app/e2e-tests/target
COPY offchain-primitives ./offchain-primitives
COPY --from=chef /app/offchain-primitives/target /app/offchain-primitives/target
COPY gsy-community-client ./gsy-community-client
COPY --from=chef /app/gsy-community-client/target /app/gsy-community-client/target

WORKDIR /app/e2e-tests
RUN cargo build --release --bin trade_executor

FROM chef AS runtime

#RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/app
COPY --from=builder /app/e2e-tests/target/release/trade_executor .
COPY --from=builder /app/e2e-tests/features ./features

ENV GSY_NODE_URL="ws://gsy-node:9944"
ENV ORDERBOOK_SERVICE_URL="http://gsy-orderbook:8080"
ENV RUST_LOG="info"

CMD ["./trade_executor"]
