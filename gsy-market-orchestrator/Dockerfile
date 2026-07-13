FROM rustlang/rust:nightly

ADD gsy-market-orchestrator /var/www/gsy-market-orchestrator/
ADD offchain-primitives /var/www/offchain-primitives/

RUN rm -rf /var/www/gsy-market-orchestrator/target/

WORKDIR /var/www/gsy-market-orchestrator/

RUN cargo build --release

ENTRYPOINT ["/var/www/gsy-market-orchestrator/target/release/gsy-market-orchestrator"]
