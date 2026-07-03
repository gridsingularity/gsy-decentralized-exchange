# Four-node PoA deployment

This document describes how to deploy the `gsy-node` chain as a permissioned
**Proof-of-Authority** network of **4 validators** plus **1 archive node**, and
the hardware, storage, networking and key-management choices behind it.

It corresponds to the `four-node-poa` chain spec / runtime preset added to the
node (`--chain four-node-poa`, built with `--features four-node-poa`).

## Topology at a glance

| Role            | Count | Validator | Keeps full history | Disk          |
|-----------------|-------|-----------|--------------------|---------------|
| Validator       | 4     | yes       | no (pruned)        | ~20 GB (HDD)  |
| Archive node    | 1     | no        | yes (archive)      | ~100 GB (SSD) |

- **Consensus:** Aura (block authoring) + GRANDPA (finality).
- **Block time:** 15 s (see [Block time](#block-time)).
- **Finality:** with 4 authorities, GRANDPA needs **3** online to finalize, so
  the network tolerates **1** faulty/offline validator. Losing two halts
  finality — plan host redundancy accordingly.

## Hardware requirements (per node)

Sized for a low-traffic chain (**< 3 transactions/minute on average**).

| Resource | Validator                      | Archive node              |
|----------|--------------------------------|---------------------------|
| CPU      | 2 cores (1 fast core matters)  | 2 cores                   |
| RAM      | 8 GB                           | 8 GB                      |
| Disk     | **20 GB**, plain HDD acceptable | **100 GB**, SSD preferred |
| Network  | reachable from the other nodes over the VPN | same             |

Notes:

- **RAM is set at 8 GB on purpose.** The large DB / trie caches (see launch
  flags) keep the small pruned working set in memory so that a plain **HDD** is
  rarely touched for random reads. This is what makes HDD viable at this load.
- A cheap **SATA SSD** is strongly recommended over HDD for validators: it gives
  orders-of-magnitude more IOPS for a few dollars and removes the risk of a disk
  stall causing a validator to miss its Aura slot. HDD is supported, not ideal.
- CPU core *count* barely matters here; Substrate block import is largely
  single-threaded, so clock speed / AVX2 matter more than cores.

## Block time

The block time is **feature-gated** in the runtime
(`gsy-node/runtime/src/lib.rs`):

- Default build (local/dev chains, **e2e docker compose**): **6 s** — unchanged.
- `four-node-poa` build: **15 s**.

The slower 15 s cadence produces ~5,760 blocks/day instead of ~14,400, which
cuts per-block storage overhead and DB commit frequency ~2.5×. That reduces both
disk growth and the IOPS pressure that would otherwise make HDDs marginal.

> The slot duration **cannot be changed after the chain has started**. It is
> fixed at genesis, so this must be decided before launching the network.

Trade-off: a transaction waits up to 15 s for inclusion and finality spans
several blocks, so settlement is slower than on the 6 s dev chain. This is
acceptable for batch-style settlement; do not lower it expecting snappy
interactive trading without re-evaluating.

## Storage and pruning

Disk growth is bounded by pruning. Without it, finalized block bodies accumulate
forever.

**Validators (bounded ~20 GB):**

```
--database paritydb \
--state-pruning 256 \
--blocks-pruning 1000
```

- `--state-pruning 256` — keep only the last 256 blocks of historical state.
- `--blocks-pruning 1000` — discard block bodies older than 1000 finalized
  blocks. **This is the flag that bounds disk growth.** At 15 s blocks, 1000
  blocks ≈ 16.7 h of retained bodies.
- `--database paritydb` — more compact on disk than the RocksDB default.

What pruning removes (and does **not**):

- It removes **old block bodies** and **old historical state**.
- It does **not** touch the **current state** — every account, order, collateral
  entry and trade result is always fully retained. A pruned validator knows the
  complete present truth of the chain; it just cannot answer "what did this look
  like N blocks ago" or serve a fresh node syncing from genesis.

**Archive node (keeps everything):**

```
--database paritydb \
--state-pruning archive \
--blocks-pruning archive
```

The archive node is your source of truth for history (explorer/analytics,
forensic queries) and the resync source for new/recovered nodes. Growth tracks
*actual* transaction volume (not block count), so at < 3 tx/min it stays in the
low single-digit GB/year range — 100 GB lasts years.

> A single archive node is a single point of failure **for history**. Take
> periodic DB snapshots (or run a second archive node) so a disk failure does not
> lose your only full record.

## Networking over a VPN

You do **not** need to expose the P2P port (30333) to the public internet. Run
all nodes on a private VPN (e.g. WireGuard, or Tailscale/Headscale if nodes are
behind NATs in different organizations) and peer them over the VPN interface.

This flips the firewall ask from inbound TCP 30333 (hard to get approved) to
outbound-only VPN connectivity (easy). 30333 then listens only on the VPN
interface.

Pin the validator set explicitly instead of relying on public discovery:

```
--node-key-file /etc/gsy/node.key \
--listen-addr /ip4/<this-node-vpn-ip>/tcp/30333 \
--public-addr /ip4/<this-node-vpn-ip>/tcp/30333 \
--reserved-nodes /ip4/<peer-vpn-ip>/tcp/30333/p2p/<peer-PeerId> ... \
--reserved-only \
--no-mdns
```

- `--reserved-only` makes the node talk **only** to the listed reserved peers.
- `--node-key-file` gives each node a stable `PeerId` across restarts so the
  reserved-nodes list does not change.
- Keep RPC bound to localhost or the VPN; do not add `--rpc-external` unless you
  intend to expose RPC.

## Keys

The `four-node-poa` genesis preset ships with the well-known **Alice/Bob/
Charlie/Dave** dev keys for convenience. **For any real deployment you must
replace them with externally generated keys**, otherwise anyone can forge your
validators.

For each validator:

1. Generate an Aura (sr25519) and a GRANDPA (ed25519) key:

   ```
   ./target/release/gsy-node key generate --scheme sr25519   # Aura + account
   ./target/release/gsy-node key inspect --scheme ed25519 "<the seed>"  # GRANDPA
   ```

2. Build a chain spec from the preset, replace the authority/account lists with
   your generated public keys, then convert to a raw spec:

   ```
   ./target/release/gsy-node build-spec --chain four-node-poa --disable-default-bootnode > four-node-poa.json
   # edit aura/grandpa authorities + balances in four-node-poa.json
   ./target/release/gsy-node build-spec --chain four-node-poa.json --raw --disable-default-bootnode > four-node-poa.raw.json
   ```

3. Distribute `four-node-poa.raw.json` to every node and insert each validator's
   keys into its keystore:

   ```
   ./target/release/gsy-node key insert --chain four-node-poa.raw.json \
     --scheme sr25519 --suri "<aura seed>" --key-type aura
   ./target/release/gsy-node key insert --chain four-node-poa.raw.json \
     --scheme ed25519 --suri "<grandpa seed>" --key-type gran
   ```

   With external keys use `--chain four-node-poa.raw.json` in place of
   `--chain four-node-poa` in the systemd units below.

## Build

```
cd gsy-node
cargo build --release --features four-node-poa
```

The `four-node-poa` feature enables both the 15 s block time and the
`--chain four-node-poa` spec. A default `cargo build --release` (what the docker
compose uses) is unaffected and keeps 6 s blocks.

## Launch (systemd)

Example units are provided under [`deployment/`](../../deployment):

- `deployment/gsy-validator.service` — a validator (pruned, 20 GB).
- `deployment/gsy-archive.service` — the archive node (full history).
- `deployment/env.example` — per-host variables referenced by the units.

Install on each host:

```
sudo cp deployment/gsy-validator.service /etc/systemd/system/
sudo cp deployment/env.example /etc/gsy/env       # edit per host
sudo useradd --system --no-create-home gsy || true
sudo mkdir -p /etc/gsy /var/lib/gsy
sudo systemctl daemon-reload
sudo systemctl enable --now gsy-validator
```

Check progress / health:

```
journalctl -u gsy-validator -f
```
