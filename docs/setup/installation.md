# Installation

## Prerequisites

Install the following tools before running the platform:

- `git`
- `rustup` (Rust toolchain manager)
- `docker` and Docker Compose
- `node` + `npm` (needed for `gsy-contracts` Hardhat workflows)

## Clone the Repository

```bash
git clone https://github.com/gridsingularity/gsy-decentralized-exchange
cd gsy-decentralized-exchange
```

## Choose an Execution Mode

The project supports two common local modes:

- **Full stack in containers (recommended):** all services + EVM + tests via Compose.
- **Rust/Node local development:** run selected services directly from source.

For most contributors, start with Docker Compose and use source-mode only for focused debugging.

## Next Steps

- Continue with [Rust Setup](rust-setup.md) if you need local source execution.
- Continue with [Docker](docker.md) for full-stack local startup.
