FROM ubuntu:22.04

WORKDIR /root
ENV PATH=$PATH:/root/.cargo/bin

RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y cmake curl libssl-dev git clang llvm libudev-dev protobuf-compiler && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

RUN \
    curl https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path && \
    rustup install 1.78.0 && \
    rustup default 1.78.0 && \
    rustup target add wasm32-unknown-unknown --toolchain 1.78.0 && \
    rustup component add rust-src --toolchain 1.78.0 && \
    rustup install nightly-2024-04-25 && \
    rustup target add wasm32-unknown-unknown --toolchain nightly-2024-04-25

ADD gsy-node /var/www/gsy-node/
ADD primitives /var/www/primitives/

WORKDIR /var/www/gsy-node/

ENV ORDERBOOK_SERVICE_URL=http://gsy-orderbook:8080
RUN cargo build --release

EXPOSE 9944

ENTRYPOINT ["./target/release/gsy-node", "--dev", "--rpc-external"]
