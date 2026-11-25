FROM rust:1.95-bookworm AS chef
RUN cargo install cargo-chef --locked

FROM chef AS planner
WORKDIR /var/www/
COPY gsy-analytics-engine /var/www/gsy-analytics-engine/
COPY primitives /var/www/primitives/
WORKDIR /var/www/gsy-analytics-engine/
RUN cargo chef prepare --recipe-path /var/www/recipe.json

FROM chef AS builder
WORKDIR /var/www/
COPY --from=planner /var/www/recipe.json /var/www/recipe.json
# primitives manifest must exist for cook, since it's a path dependency
COPY primitives /var/www/primitives/
WORKDIR /var/www/gsy-analytics-engine/
RUN cargo chef cook --release --tests --recipe-path /var/www/recipe.json
# Now copy full source and build. No solc/foundry needed: this crate does not use ethers.
COPY gsy-analytics-engine /var/www/gsy-analytics-engine/
COPY primitives /var/www/primitives/
# Drop the artifacts cook built from the placeholder sources of the local crates
RUN rm -rf \
    target/release/.fingerprint/primitives-* \
    target/release/build/primitives-* \
    target/release/deps/primitives-* \
    target/release/deps/libprimitives* \
    target/release/.fingerprint/gsy-analytics-engine-* \
    target/release/deps/gsy_analytics_engine-* \
    target/release/deps/libgsy_analytics_engine*
# Compile the tests too, so the integration test service can run `cargo test` quickly.
RUN cargo build --release --locked && cargo test --release --locked --no-run

FROM rust:1.95-bookworm AS runtime
WORKDIR /var/www/gsy-analytics-engine
COPY --from=builder /var/www/gsy-analytics-engine/target/release/gsy-analytics-engine /usr/local/bin
EXPOSE 8081
ENTRYPOINT ["/usr/local/bin/gsy-analytics-engine"]
