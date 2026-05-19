FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the orderbook service
WORKDIR /var/www/gsy-offchain-storage
COPY ./gsy-offchain-storage/Cargo.lock ./gsy-offchain-storage/Cargo.toml /var/www/gsy-offchain-storage/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder

COPY offchain-primitives /var/www/offchain-primitives/
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY gsy-offchain-storage /var/www/gsy-offchain-storage/
COPY --from=chef /var/www/gsy-offchain-storage/target /var/www/gsy-offchain-storage/target

WORKDIR /var/www/gsy-offchain-storage/
RUN cargo build --release

FROM rustlang/rust:nightly

WORKDIR /app

COPY --from=builder /var/www/gsy-offchain-storage/target/release/gsy-offchain-storage .
COPY --from=builder /var/www/gsy-offchain-storage/configuration.yaml .
EXPOSE 8080

ENTRYPOINT ["/app/gsy-offchain-storage"]
