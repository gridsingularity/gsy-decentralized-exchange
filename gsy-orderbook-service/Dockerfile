FROM rust:latest AS chef
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

WORKDIR /var/www/gsy-orderbook-service
COPY ./gsy-orderbook-service/Cargo.lock ./gsy-orderbook-service/Cargo.toml /var/www/gsy-orderbook-service/
RUN mkdir src && touch src/main.rs
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl

RUN cargo install svm-rs && svm install 0.8.20 && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc

RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

COPY offchain-primitives /var/www/offchain-primitives/
COPY gsy-ethers-listener /var/www/gsy-ethers-listener/
COPY gsy-orderbook-service /var/www/gsy-orderbook-service/

WORKDIR /var/www/gsy-orderbook-service/
RUN cargo build --release

FROM rust:latest

RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /var/www/gsy-orderbook-service/target/release/gsy-orderbook-service .
COPY --from=builder /var/www/gsy-orderbook-service/configuration.yaml .

EXPOSE 8080
ENTRYPOINT ["/app/gsy-orderbook-service"]
