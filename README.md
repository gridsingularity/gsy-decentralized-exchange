<p align="center">
  <img src="./docs/images/Profile.png" alt="GSy Logo" title="GSy Logo" width="200" />
</p>

# GSy Decentralized Energy Exchange

The GSy Decentralized Energy Exchange (DEX) aims to revolutionize the way energy is traded and managed by leveraging the power of distributed ledger technology, such as blockchain, and advanced distributed runtime functionalities. 

The target of the decentralized energy exchange is to design, develop, and implement a robust, secure, and efficient platform for energy trading in a decentralized environment. 

The GSy Decentralized Energy Exchange can effectively facilitate peer-to-peer energy trading, optimize energy consumption and generation, and ultimately contribute to a more sustainable and resilient energy infrastructure.

## Installation Instructions

Follow the steps below to set up the GSy Decentralized Energy Exchange locally.

### Prerequisites

Ensure you have the following software installed on your system before proceeding:

1. Git - [Download and install Git](https://git-scm.com/downloads)
2. [Rust](https://www.rust-lang.org/tools/install) (Install Rust programming language and Cargo, its package manager)

### Clone the Repository
First, clone the repository to your local machine using the following command:
```sh
git clone https://github.com/gridsingularity/gsy-decentralized-exchange
```
### Navigate to the Project Directory
Change to the project's directory using the command:
```sh
cd gsy-decentralized-exchange
```

### Build & Run the Project services
#### GSy Node
```sh
cd gsy-node
```
Use the following command to build the node without launching it:

```sh
cargo build --release
```

Use Rust's native `cargo run` command to build and launch the gsy-node:
```sh
cargo run --release -- --dev --tmp
```
or if you have already built using `cargo build` you can launch the gsy-node using the following command:
```sh
./target/release/gsy-node --dev --tmp
```

### Run Service using Docker Compose

First, install [Docker](https://docs.docker.com/get-docker/) and
[Docker Compose](https://docs.docker.com/compose/install/).

Build and tag the docker image:

```bash
docker build -t gsy_dex_image .
docker tag gsy_dex_image:latest gsy_dex_image:staging
```
and start docker-compose:

```bash
docker-compose up
```