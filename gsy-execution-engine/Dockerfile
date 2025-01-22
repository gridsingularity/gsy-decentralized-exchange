FROM rust:1.80

WORKDIR /var/www/gsy-execution-engine

COPY . /var/www/gsy-execution-engine

RUN cargo build --release

ENTRYPOINT ["/var/www/gsy-execution-engine/target/release/gsy-execution-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944", "30"]
