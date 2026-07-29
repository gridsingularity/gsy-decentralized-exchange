FROM rust:1.89.0-bookworm AS base

RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install svm-rs --version 0.5.25 --locked && svm install 0.8.20 \
    && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

RUN cargo install cargo-chef --locked

FROM base AS planner
WORKDIR /var/www/
COPY gsy-market-orchestrator /var/www/gsy-market-orchestrator/
COPY primitives /var/www/primitives/
WORKDIR /var/www/gsy-market-orchestrator/
RUN cargo chef prepare --recipe-path /var/www/recipe.json

FROM base AS builder
WORKDIR /var/www/
COPY --from=planner /var/www/recipe.json /var/www/recipe.json
# primitives manifest must exist for cook, since it's a path dependency
COPY primitives /var/www/primitives/
WORKDIR /var/www/gsy-market-orchestrator/
RUN cargo chef cook --release --recipe-path /var/www/recipe.json
# Now copy full source and build
COPY gsy-market-orchestrator /var/www/gsy-market-orchestrator/
COPY primitives /var/www/primitives/
RUN cargo build --release

ENTRYPOINT ["/var/www/gsy-market-orchestrator/target/release/gsy-market-orchestrator"]