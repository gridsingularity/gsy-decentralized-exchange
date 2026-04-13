# Rust Setup

This section is required when developing Rust services locally.

## Install Toolchain

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Validate Toolchain

```bash
rustup default stable
rustup update
cargo --version
rustc --version
```

## Optional: Additional Tooling

Useful tools for local development:

```bash
cargo install cargo-watch
cargo install cargo-nextest
```

## Notes

The current refactored architecture does **not** require building a Substrate node.
Rust is used for off-chain services and test binaries.
