# Build

## Build All Rust Services

From repository root:

```bash
cargo build --release --manifest-path gsy-market-orchestrator/Cargo.toml
cargo build --release --manifest-path gsy-matching-engine/Cargo.toml
cargo build --release --manifest-path gsy-execution-engine/Cargo.toml
cargo build --release --manifest-path gsy-orderbook-service/Cargo.toml
cargo build --release --manifest-path gsy-community-client/Cargo.toml
```

## Build Smart Contracts

```bash
cd gsy-contracts
npm install
npx hardhat compile
```

## Build E2E Runner

```bash
cargo build --release --manifest-path e2e-tests/Cargo.toml --bin trade_executor
```

## Build Container Images

```bash
docker compose build
```
