FROM ubuntu:22.04

WORKDIR /root
ENV PATH=$PATH:/root/.cargo/bin

RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y cmake curl libssl-dev git clang llvm libudev-dev protobuf-compiler && \
    apt-get clean && \
    curl https://sh.rustup.rs -sSf | sh -s -- -y
RUN rustup default stable && \
    rustup update && \
    rustup update nightly && \
    rustup component add rust-src && \
    rustup target add wasm32-unknown-unknown

ADD gsy-node /var/www/gsy-node/
ADD primitives /var/www/primitives/

WORKDIR /var/www/gsy-node/

ENV ORDERBOOK_SERVICE_URL=http://gsy-orderbook:8080
RUN cargo build --release

EXPOSE 9944

ENTRYPOINT ["./target/release/gsy-node", "--dev", "--rpc-external"]
