FROM rust:1.80

ADD gsy-matching-engine /var/www/gsy-matching-engine/

# build for release
RUN rm -rf /var/www/gsy-matching-engine/target/

WORKDIR /var/www/gsy-matching-engine/

RUN cargo build --release

# set the startup command to run your binary
ENTRYPOINT ["/var/www/gsy-matching-engine/target/release/gsy-matching-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944"]
