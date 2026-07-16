FROM rust:1.95 AS chef
RUN cargo install cargo-chef --locked

WORKDIR /var/www
COPY ./primitives/Cargo.lock ./primitives/Cargo.toml /var/www/primitives/
RUN mkdir -p /var/www/primitives/src && touch /var/www/primitives/src/lib.rs

COPY ./gsy-ethers-listener/Cargo.lock ./gsy-ethers-listener/Cargo.toml /var/www/gsy-ethers-listener/
RUN mkdir -p /var/www/gsy-ethers-listener/src && touch /var/www/gsy-ethers-listener/src/lib.rs

WORKDIR /var/www/gsy-offchain-storage
COPY ./gsy-offchain-storage/Cargo.lock ./gsy-offchain-storage/Cargo.toml /var/www/gsy-offchain-storage/
RUN mkdir -p /var/www/gsy-offchain-storage/src \
    && touch /var/www/gsy-offchain-storage/src/lib.rs /var/www/gsy-offchain-storage/src/main.rs
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl

RUN cargo install svm-rs --version 0.5.25 --locked && svm install 0.8.20 && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc

RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

COPY primitives /var/www/primitives/

COPY gsy-ethers-listener /var/www/gsy-ethers-listener/

COPY gsy-offchain-storage /var/www/gsy-offchain-storage/
COPY --from=chef /var/www/gsy-offchain-storage/target /var/www/gsy-offchain-storage/target

WORKDIR /var/www/gsy-offchain-storage/
RUN rm -rf \
    target/release/.fingerprint/primitives-* \
    target/release/.fingerprint/gsy-* \
    target/release/build/primitives-* \
    target/release/build/gsy-* \
    target/release/deps/primitives-* \
    target/release/deps/libprimitives* \
    target/release/deps/gsy_* \
    target/release/deps/libgsy_*
RUN cargo build --release

FROM rust:1.95

WORKDIR /app
COPY --from=builder /var/www/gsy-offchain-storage/target/release/gsy-offchain-storage .
COPY --from=builder /var/www/gsy-offchain-storage/configuration.yaml .

EXPOSE 8080
ENTRYPOINT ["/app/gsy-offchain-storage"]
