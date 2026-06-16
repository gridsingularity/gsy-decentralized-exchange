FROM rust:1.95 AS chef
RUN cargo install cargo-chef

WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN mkdir src && touch src/lib.rs
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

WORKDIR /var/www/gsy-ethers-listener
COPY ./gsy-ethers-listener/Cargo.lock ./gsy-ethers-listener/Cargo.toml /var/www/gsy-ethers-listener/
RUN mkdir src && touch src/lib.rs
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

WORKDIR /var/www/gsy-offchain-storage
COPY ./gsy-offchain-storage/Cargo.lock ./gsy-offchain-storage/Cargo.toml /var/www/gsy-offchain-storage/
RUN mkdir src && touch src/lib.rs src/main.rs
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl

RUN cargo install svm-rs --version 0.5.25 --locked && svm install 0.8.20 && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc

RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

COPY offchain-primitives /var/www/offchain-primitives/
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY gsy-ethers-listener /var/www/gsy-ethers-listener/
COPY --from=chef /var/www/gsy-ethers-listener/target /var/www/gsy-ethers-listener/target

COPY gsy-offchain-storage /var/www/gsy-offchain-storage/
COPY --from=chef /var/www/gsy-offchain-storage/target /var/www/gsy-offchain-storage/target

WORKDIR /var/www/gsy-offchain-storage/
RUN cargo build --release

FROM rust:1.95

WORKDIR /app
COPY --from=builder /var/www/gsy-offchain-storage/target/release/gsy-offchain-storage .
COPY --from=builder /var/www/gsy-offchain-storage/configuration.yaml .

EXPOSE 8080
ENTRYPOINT ["/app/gsy-offchain-storage"]
