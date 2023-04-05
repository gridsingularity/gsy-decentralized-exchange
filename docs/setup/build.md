## Build

The `cargo run` command will perform an initial build. Use the following command to build the node
without launching it:

```sh
cd gsy-node
cargo build --release
```
## Embedded Docs

Once the project has been built, the following command can be used to explore all parameters and
subcommands:

```sh
./target/release/gsy-node -h
```

## Run

Use Rust's native `cargo` command to build and launch the gsy-node:
```sh
cd gsy-node
cargo run --release -- --dev --tmp
```
or if you have already built using `cargo build` you can launch the gsy-node using the following command:
```sh
./target/release/gsy-node --dev --tmp
```

You should always use the `--release` flag to build optimized artifacts.

The command-line options specify how you want the running node to operate. 
In this case, the `--dev` option specifies that the node runs in development mode using the predefined development chain specification. 

By default, this option also deletes all active data—such as keys, the blockchain database, and networking information when you stop the node by pressing Control-c. 

Using the `--dev` option ensures that you have a clean working state any time you stop and restart the node.

Verify your node is up and running successfully by reviewing the output displayed in the terminal.

The terminal should display output similar to this:

```sh
2022-08-16 13:43:58 Substrate Node    
2022-08-16 13:43:58 ✌️  version 4.0.0-dev-de262935ede    
2022-08-16 13:43:58 ❤️  by Substrate DevHub <https://github.com/substrate-developer-hub>, 2017-2022    
2022-08-16 13:43:58 📋 Chain specification: Development
2022-08-16 13:43:58 🏷  Node name: limping-oatmeal-7460    
2022-08-16 13:43:58 👤 Role: AUTHORITY    
2022-08-16 13:43:58 💾 Database: RocksDb at /var/folders/2_/g86ns85j5l7fdnl621ptzn500000gn/T/substrate95LPvM/chains/dev/db/full    
2022-08-16 13:43:58 ⛓  Native runtime: node-template-100 (node-template-1.tx1.au1)
2022-08-16 13:43:58 🔨 Initializing Genesis block/state (state: 0xf6f5…423f, header-hash: 0xc665…cf6a)
2022-08-16 13:43:58 👴 Loading GRANDPA authority set from genesis on what appears to be first startup.
2022-08-16 13:43:59 Using default protocol ID "sup" because none is configured in the chain specs
2022-08-16 13:43:59 🏷  Local node identity is: 12D3KooWCu9uPCYZVsayaCKLdZLF8CmqiHkX2wHsAwSYVc2CxmiE
...
...
...
...
2022-08-16 13:54:26 💤 Idle (0 peers), best: #3 (0xcdac…26e5), finalized #1 (0x107c…9bae), ⬇ 0 ⬆ 0
```

If the number after finalized is increasing, your blockchain is producing new blocks and reaching consensus about the state they describe.

## Single-Node Development Chain

This command will start the single-node development chain with persistent state:

```bash
./target/release/gsy-node --dev
```

Purge the development chain's state:

```bash
./target/release/gsy-node purge-chain --dev
```

Start the development chain with detailed logging:

```bash
RUST_BACKTRACE=1 ./target/release/gsy-node -ldebug --dev
```