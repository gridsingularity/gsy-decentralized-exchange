FROM rustlang/rust:nightly

WORKDIR /var/www/gsy-execution-engine

COPY ./gsy-execution-engine /var/www/gsy-execution-engine
COPY ./offchain-primitives /var/www/offchain-primitives

RUN cargo build --release

ENTRYPOINT ["/var/www/gsy-execution-engine/target/release/gsy-execution-engine", "web3", "http://gsy-orderbook", "8080", "ws://gsy-node", "9944", "30", "900", "0.10"]
