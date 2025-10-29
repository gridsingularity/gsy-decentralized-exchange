FROM ubuntu:22.04 AS chef
WORKDIR /root
ENV PATH=$PATH:/root/.cargo/bin
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y cmake curl libssl-dev git clang llvm libudev-dev protobuf-compiler && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/* && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path && \
    rustup install 1.85.0 && \
    rustup default 1.85.0 && \
    rustup target add wasm32-unknown-unknown --toolchain 1.85.0 && \
    rustup component add rust-src --toolchain 1.85.0 && \
    rustup install nightly-2024-04-25 && \
    rustup target add wasm32-unknown-unknown --toolchain nightly-2024-04-25 && \
    cargo install cargo-chef

FROM chef AS planner
# First prepare dependencies of primitives
WORKDIR /var/www/primitives
COPY ./primitives/Cargo.lock ./primitives/Cargo.toml /var/www/primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the node
WORKDIR /var/www/gsy-node
COPY ./gsy-node/Cargo.lock ./gsy-node/Cargo.toml /var/www/gsy-node/
COPY ./gsy-node/node/Cargo.toml /var/www/gsy-node/node/
COPY ./gsy-node/runtime/Cargo.toml /var/www/gsy-node/runtime/
COPY ./gsy-node/modules/gsy-collateral/Cargo.toml /var/www/gsy-node/modules/gsy-collateral/
COPY ./gsy-node/modules/orderbook-registry/Cargo.toml /var/www/gsy-node/modules/orderbook-registry/
COPY ./gsy-node/modules/orderbook-worker/Cargo.toml /var/www/gsy-node/modules/orderbook-worker/
COPY ./gsy-node/modules/trades-settlement/Cargo.toml /var/www/gsy-node/modules/trades-settlement/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY gsy-node /var/www/gsy-node/
COPY primitives /var/www/primitives/
COPY --from=planner /var/www/primitives/target /var/www/primitives/target
COPY --from=planner /var/www/gsy-node/target /var/www/gsy-node/target
WORKDIR /var/www/gsy-node/
ENV ORDERBOOK_SERVICE_URL=http://gsy-orderbook:8080
RUN cargo build --release

FROM chef AS runtime

WORKDIR /var/www/gsy-node/
COPY --from=builder /var/www/gsy-node/target /var/www/gsy-node/target
ENV ORDERBOOK_SERVICE_URL=http://gsy-orderbook:8080
EXPOSE 9944
ENTRYPOINT ["./target/release/gsy-node", "--dev", "--rpc-external"]
