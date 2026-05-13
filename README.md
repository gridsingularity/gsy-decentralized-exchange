<p align="center">
  <img src="./docs/images/Profile.png" alt="GSy Logo" title="GSy Logo" width="200" />
</p>

# Grid Singularity Decentralized Energy Exchange (GSY DEX v.2)
The Grid Singularity Decentralized Energy Exchange (GSY DEX) is developed by [Grid Singularity](https://gridsingularity.com/) as an open source GPL v.3 codebase (see [Licensing](https://gridsingularity.github.io/gsy-e/licensing/)) to model, simulate, optimise and deploy interconnected, grid-aware energy marketplaces. Grid Singularity has been proclaimed the [World Tech Pioneer by the World Economic Forum](https://www.weforum.org/organizations/grid-singularity-gmbh-gsy-gmbh) and is also known as a co-founder of the [Energy Web Foundation](https://www.energyweb.org/) that gathers leading energy and sustainability organisations globally, co-developing a shared decentralised digital trust platform. This is the branch for GSY DEX v.2.

## Installation Instructions

Follow the steps below to set up the GSY Decentralized Energy Exchange locally.

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
#### GSY Node
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