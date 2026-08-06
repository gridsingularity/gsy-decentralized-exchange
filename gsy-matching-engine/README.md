# GSY Matching Engine

The matching engine runs against the EVM/Web3 stack and reads orders from the configured off-chain storage transport.

Build the Docker image:

```sh
docker build -t matching_engine -f Dockerfile ..
```

Run locally against the default services:

```sh
docker run --rm --name matching_engine matching_engine web3
```

Only the EVM/Web3 command path is supported for local and integration runs.
