FROM rust:1.89.0-bookworm AS chef
RUN cargo install cargo-chef --locked

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

# Then dependencies of the e2e tests
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

WORKDIR /usr/src/app
COPY --from=builder /app/e2e-tests/target/release/trade_executor .
COPY --from=builder /app/e2e-tests/features ./features

ENV EVM_NODE_URL="ws://anvil:8545"
ENV OFFCHAIN_STORAGE_URL="http://gsy-offchain-storage:8080"
ENV RUST_LOG="info"

CMD ["./trade_executor"]
