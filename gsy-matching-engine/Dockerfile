FROM rust:1.89.0-bookworm AS chef
RUN cargo install cargo-chef --locked

# First prepare dependencies of primitives
WORKDIR /var/www/primitives
COPY ./primitives/Cargo.lock ./primitives/Cargo.toml /var/www/primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the matching engine
WORKDIR /var/www/gsy-matching-engine
COPY ./gsy-matching-engine/Cargo.lock ./gsy-matching-engine/Cargo.toml /var/www/gsy-matching-engine/
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

COPY ./gsy-matching-engine /var/www/gsy-matching-engine
COPY --from=chef /var/www/gsy-matching-engine/target /var/www/gsy-matching-engine/target

WORKDIR /var/www/gsy-matching-engine
RUN rm -rf \
    target/release/.fingerprint/primitives-* \
    target/release/build/primitives-* \
    target/release/deps/primitives-* \
    target/release/deps/libprimitives*
RUN cargo build --release

FROM rust:1.89.0-bookworm AS runtime
WORKDIR /var/www/gsy-matching-engine
COPY --from=builder /var/www/gsy-matching-engine/target/release/gsy-matching-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-matching-engine", "web3", "http://gsy-offchain-storage", "8080", "ws://anvil", "8545"]
