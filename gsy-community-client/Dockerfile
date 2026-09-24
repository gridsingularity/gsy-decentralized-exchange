FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin gsy-community-client

FROM rust:1.95-bookworm AS runtime
WORKDIR /var/www/gsy-community-client
COPY --from=builder /var/www/target/release/gsy-community-client /usr/local/bin
ENTRYPOINT ["/usr/local/bin/gsy-community-client"]
