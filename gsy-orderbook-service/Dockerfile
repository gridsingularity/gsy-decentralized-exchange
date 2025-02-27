FROM rust:1.84.1-bookworm AS builder

ADD gsy-orderbook-service /var/www/gsy-orderbook-service/
ADD offchain-primitives /var/www/offchain-primitives/

# build for release
RUN rm -rf /var/www/gsy-orderbook-service/target/

WORKDIR /var/www/gsy-orderbook-service/

RUN cargo build --release

RUN cp ./target/release/gsy-orderbook-service /var/www/gsy-orderbook-service/
RUN rm -rf target/


FROM debian:bookworm-slim
WORKDIR /app

COPY --from=builder /var/www/gsy-orderbook-service/gsy-orderbook-service .
COPY --from=builder /var/www/gsy-orderbook-service/configuration.yaml .
EXPOSE 8080

ENTRYPOINT ["/app/gsy-orderbook-service"]
