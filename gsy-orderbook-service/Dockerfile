FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the orderbook service
WORKDIR /var/www/gsy-orderbook-service
COPY ./gsy-orderbook-service/Cargo.lock ./gsy-orderbook-service/Cargo.toml /var/www/gsy-orderbook-service/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder

COPY offchain-primitives /var/www/offchain-primitives/
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY gsy-orderbook-service /var/www/gsy-orderbook-service/
COPY --from=chef /var/www/gsy-orderbook-service/target /var/www/gsy-orderbook-service/target

WORKDIR /var/www/gsy-orderbook-service/
RUN cargo build --release

FROM rustlang/rust:nightly

WORKDIR /app

COPY --from=builder /var/www/gsy-orderbook-service/target/release/gsy-orderbook-service .
COPY --from=builder /var/www/gsy-orderbook-service/configuration.yaml .
EXPOSE 8080

ENTRYPOINT ["/app/gsy-orderbook-service"]
