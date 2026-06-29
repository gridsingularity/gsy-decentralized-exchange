FROM rust:1.89.0-bookworm

RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl
RUN cargo install svm-rs --version 0.5.25 --locked && svm install 0.8.20 && ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

ADD gsy-market-orchestrator /var/www/gsy-market-orchestrator/
ADD offchain-primitives /var/www/offchain-primitives/

RUN rm -rf /var/www/gsy-market-orchestrator/target/

WORKDIR /var/www/gsy-market-orchestrator/

RUN cargo build --release

ENTRYPOINT ["/var/www/gsy-market-orchestrator/target/release/gsy-market-orchestrator"]
