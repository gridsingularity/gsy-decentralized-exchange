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

The provided `cargo run` command will launch a temporary node and its state will be discarded after
you terminate the process. After the project has been built, there are other ways to launch the
node.

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