FROM rust:latest

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    git \
    curl

RUN cargo install svm-rs
RUN svm install 0.8.20
RUN ln -s /root/.svm/0.8.20/solc-0.8.20 /usr/local/bin/solc

RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

WORKDIR /app
COPY . .

CMD ["cargo", "test", "--test", "integration_test"]
