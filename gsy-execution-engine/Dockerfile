FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef

# First prepare dependencies of offchain-primitives
WORKDIR /var/www/offchain-primitives
COPY ./offchain-primitives/Cargo.lock ./offchain-primitives/Cargo.toml /var/www/offchain-primitives/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# And then dependencies of the execution engine
WORKDIR /var/www/gsy-execution-engine
COPY ./gsy-execution-engine/Cargo.lock ./gsy-execution-engine/Cargo.toml /var/www/gsy-execution-engine/
RUN cargo chef prepare --recipe-path recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM chef AS builder
COPY ./offchain-primitives /var/www/offchain-primitives
COPY --from=chef /var/www/offchain-primitives/target /var/www/offchain-primitives/target

COPY ./gsy-execution-engine /var/www/gsy-execution-engine
COPY --from=chef /var/www/gsy-execution-engine/target /var/www/gsy-execution-engine/target

WORKDIR /var/www/gsy-execution-engine
RUN cargo build --release

FROM rustlang/rust:nightly AS runtime
WORKDIR /var/www/gsy-execution-engine
COPY --from=builder /var/www/gsy-execution-engine/target/release/gsy-execution-engine /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-execution-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944", "30", "900", "0.10"]
