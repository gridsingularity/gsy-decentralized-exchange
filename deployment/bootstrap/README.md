# Demo bootstrap

One-shot container for the local demo (`docker-compose.local-demo.yml`). It
performs the on-chain registrations the chain needs before any trading can
happen, then exits.

## What it does

Connects to `gsy-node` and, signing with the dev **sudo** key (`//Alice`),
submits two root-only extrinsics via `sudo.sudo(...)`:

1. `gsyCollateral.registerUser(<account>)` for each account in
   `REGISTER_USER_SURIS` — required because `orderbook_registry.insert_orders`
   rejects orders from unregistered users, and the community client signs its
   orders as `//Alice`.
2. `gsyCollateral.registerExchangeOperator(<operator>)` — required because the
   market orchestrator refuses to create markets until its signer account is a
   registered exchange operator.

It is **idempotent**: it checks the `registeredUser` / `registeredExchangeOperator`
storage first and tolerates `AlreadyRegistered`, so re-running (or running
against a chain that already has state) is a no-op.

## Configuration (`.env/gsy-bootstrap.env`)

| Variable | Default | Meaning |
|----------|---------|---------|
| `NODE_URL` | `ws://gsy-node:9944` | Node RPC endpoint. |
| `SUDO_SURI` | `//Alice` | Dev sudo key that signs the wrapping `sudo` calls. |
| `REGISTER_USER_SURIS` | `//Alice` | Comma-separated SURIs to register as trading users. |
| `OPERATOR_SURI` | value of `SUDO_SURI` | Account to register as the exchange operator (the orchestrator's signer). |
| `CONNECT_RETRIES` | `60` | Node connection attempts (2s apart) before giving up. |

## In the compose graph

`gsy-bootstrap` depends on `gsy-node` being healthy; `gsy-market-orchestrator`
and `gsy-community-client` depend on `gsy-bootstrap` completing successfully, so
the stack comes up already registered and ready to trade.

> Dev keys only. This is for the local demo; a real deployment registers real
> accounts through its own governance/sudo process.

## Note on `@polkadot/api` warnings

You may see lines like `Unsupported unsigned extrinsic version 5` /
`RPC-CORE: getBlock ... failed` during the run. These are **cosmetic**: the
pinned `@polkadot/api` (v12) can't pretty-decode the runtime's newer extrinsic
envelope (format v5) when it reads a block, but transaction *submission* and
event-based error detection do not depend on that — the registrations still
land (each logs `included in <block>`). Bump `@polkadot/api` in `package.json`
to a version with extrinsic-v5 support if you want to silence them.
