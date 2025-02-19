FROM rust:1.80

ADD gsy-orderbook-service /var/www/gsy-orderbook-service/
ADD offchain-primitives /var/www/offchain-primitives/

# build for release
RUN rm -rf /var/www/gsy-orderbook-service/target/

WORKDIR /var/www/gsy-orderbook-service/

RUN cargo build --release

EXPOSE 8080

ENTRYPOINT ["./target/release/gsy-orderbook-service"]
