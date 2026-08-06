FROM rust:1.89.0-bookworm AS chef
RUN cargo install cargo-chef --locked

# First prepare dependencies of primitives
WORKDIR /var/www/primitives
COPY ./primitives/Cargo.lock ./primitives/Cargo.toml /var/www/primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the execution engine
WORKDIR /var/www/gsy-execution-engine
COPY ./gsy-execution-engine/Cargo.lock ./gsy-execution-engine/Cargo.toml /var/www/gsy-execution-engine/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl
RUN cargo install svm-rs --version 0.5.25 --locked && svm install 0.8.20 && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

COPY ./primitives /var/www/primitives
COPY --from=chef /var/www/primitives/target /var/www/primitives/target

COPY ./gsy-execution-engine /var/www/gsy-execution-engine
COPY --from=chef /var/www/gsy-execution-engine/target /var/www/gsy-execution-engine/target

WORKDIR /var/www/gsy-execution-engine
RUN rm -rf \
    target/release/.fingerprint/primitives-* \
    target/release/build/primitives-* \
    target/release/deps/primitives-* \
    target/release/deps/libprimitives*
RUN cargo build --release

FROM rust:1.89.0-bookworm AS runtime
WORKDIR /var/www/gsy-execution-engine
COPY --from=builder /var/www/gsy-execution-engine/target/release/gsy-execution-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-execution-engine", "web3", "http://gsy-offchain-storage", "8080", "ws://anvil", "8545", "30", "3600", "0.10"]
