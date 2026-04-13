# Testing

## Contract Tests

```bash
cd gsy-contracts
npm install
npx hardhat test
```

## Rust Integration Tests

Run per component:

```bash
cargo test --manifest-path gsy-market-orchestrator/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-matching-engine/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-execution-engine/Cargo.toml --test evm_integration
cargo test --manifest-path gsy-orderbook-service/Cargo.toml --test api
cargo test --manifest-path gsy-community-client/Cargo.toml --tests
```

## End-to-End Cucumber Tests

```bash
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit e2e-tests
```

Current e2e suite validates:

- Standard bid/offer matching and on-chain settlement.
- Preference-based matching behavior and preferred price selection.
- Penalty submission from execution engine.
