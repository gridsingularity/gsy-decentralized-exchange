FROM rust:1.95-bookworm

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    git \
    curl

RUN cargo install svm-rs --version 0.5.25 --locked
RUN svm install 0.8.20
RUN ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc

RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN for i in 1 2 3 4 5; do foundryup && break || { echo "foundryup attempt $i failed, retrying..."; sleep $((i * 5)); }; done

WORKDIR /app
COPY . .
