FROM ghcr.io/gridsingularity/gsy-rust-base:latest AS builder
COPY . .
# Compile the tests without running them, so the container is ready to
# `cargo test --release` fast at run time (compose supplies working_dir and command).
RUN cargo test --release -p gsy-ethers-listener --no-run
