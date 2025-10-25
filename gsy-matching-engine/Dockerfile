FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the matching engine
WORKDIR /var/www/gsy-matching-engine
COPY ./gsy-matching-engine/Cargo.lock ./gsy-matching-engine/Cargo.toml /var/www/gsy-matching-engine/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY ./offchain-primitives /var/www/offchain-primitives
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY ./gsy-matching-engine /var/www/gsy-matching-engine
COPY --from=chef /var/www/gsy-matching-engine/target /var/www/gsy-matching-engine/target

WORKDIR /var/www/gsy-matching-engine
RUN cargo build --release

FROM rustlang/rust:nightly AS runtime
WORKDIR /var/www/gsy-matching-engine
COPY --from=builder /var/www/gsy-matching-engine/target/release/gsy-matching-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-matching-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944"]
