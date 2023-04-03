FROM ubuntu:20.04

ENV DEBIAN_FRONTEND=noninteractive

WORKDIR /root

ENV PATH=$PATH:/root/.cargo/bin

RUN   apt-get update && \
      apt-get upgrade -y && \
      apt-get install -y cmake curl libssl-dev git clang && \
      apt-get clean && \
      curl https://sh.rustup.rs -sSf | sh -s -- -y && \
      rustup toolchain install nightly && \
      rustup target add wasm32-unknown-unknown --toolchain nightly && \
      rustup default stable
