FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
RUN cargo build --release --bin gsy-offchain-storage

FROM rust:1.95-bookworm

WORKDIR /app
COPY --from=builder /var/www/target/release/gsy-offchain-storage .
COPY --from=builder /var/www/gsy-offchain-storage/configuration.yaml .

EXPOSE 8080
ENTRYPOINT ["/app/gsy-offchain-storage"]
