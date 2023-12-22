FROM ubuntu:20.04

ENV DEBIAN_FRONTEND=noninteractive

WORKDIR /root

ENV PATH=$PATH:/root/.cargo/bin

RUN   apt-get update && \
      apt-get upgrade -y && \
      apt-get install -y cmake curl libssl-dev git clang && \
      apt-get clean 
RUN   curl https://sh.rustup.rs -sSf | sh -s -- -y 
RUN   rustup toolchain install stable
RUN   rustup target add wasm32-unknown-unknown
RUN   rustup default stable
