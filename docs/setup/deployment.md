# PoA deployment

This document describes how to deploy the `gsy-node` chain as a permissioned
**Proof-of-Authority** network, and the hardware, storage, networking and
key-management choices behind it.

The network starts small — **2 validators** plus **1 archive node** — and is
designed to **grow**: further validators can be added (or removed) on the live
chain without a re-genesis, via the `validator-set` pallet. See
[Adding validators to a running network](#adding-validators-to-a-running-network).

It corresponds to the `poa` chain spec / runtime preset (`--chain poa`, built
with `--features poa`).

## Topology at a glance

| Role            | Count (initial) | Validator | Keeps full history | Disk          |
|-----------------|-----------------|-----------|--------------------|---------------|
| Validator       | 2 (growable)    | yes       | no (pruned)        | ~20 GB (HDD)  |
| Archive node    | 1               | no        | yes (archive)      | ~100 GB (SSD) |

- **Consensus:** Aura (block authoring) + GRANDPA (finality).
- **Block time:** 15 s (see [Block time](#block-time)).
- **Authority set:** managed on-chain by the `validator-set` pallet and applied
  by `pallet-session`. It is **not** fixed at genesis, so you can add/remove
  validators later.

> ### ⚠️ Fault tolerance vs. validator count — read this before launch
>
> GRANDPA finalizes a block only with **more than 2/3** of the authorities
> voting. That means the number of validators you can lose and still finalize is
> `floor((N-1)/3)`:
>
> | Validators (N) | Needed online to finalize | Tolerated offline |
> |----------------|---------------------------|-------------------|
> | **2**          | 2                         | **0**             |
> | 3              | 3                         | **0**             |
> | **4**          | 3                         | **1**             |
> | 5–6            | ⌈2N/3⌉+…                  | 1                 |
> | 7              | 5                         | 2                 |
>
> **A 2-validator network tolerates zero failures for finality.** If either
> validator goes down, Aura keeps producing blocks (at half rate) but GRANDPA
> **stops finalizing** until it returns. Starting at 2 is fine for bring-up, but
> **grow to 4 as soon as practical** — that is the first count that survives one
> validator being offline. Going 2 → 3 does *not* improve fault tolerance; 4 does.

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
- `poa` build: **15 s**.

The slower 15 s cadence produces ~5,760 blocks/day instead of ~14,400, which
cuts per-block storage overhead and DB commit frequency ~2.5×. That reduces both
disk growth and the IOPS pressure that would otherwise make HDDs marginal.

> The slot duration **cannot be changed after the chain has started**. It is
> fixed at genesis, so this must be decided before launching the network.
> (Validator *count* can change later; block *time* cannot.)

Trade-off: a transaction waits up to 15 s for inclusion and finality spans
several blocks, so settlement is slower than on the 6 s dev chain. This is
acceptable for batch-style settlement; do not lower it expecting snappy
interactive trading without re-evaluating.

## Sessions

Validator-set changes take effect at **session boundaries**. The session length
is set in the runtime (`SessionPeriod`) to **1 hour**. So when you add or remove
a validator, the change becomes active at the next session rotation — within an
hour, not immediately. This cadence is deliberate: validator changes are
expected to be infrequent, and longer sessions mean fewer authority-set
rotations to propagate.

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

## Keys and genesis

The `poa` genesis preset ships with the well-known **Alice/Bob** dev keys for
convenience (2 validators, with Alice as `sudo`). **For any real deployment you
must replace them with externally generated keys**, otherwise anyone can forge
your validators and take over the chain via sudo.

With `pallet-session` in place, the genesis authority sets are **not** listed
under `aura`/`grandpa` anymore. They are derived from the `session` keys, and the
initial active set is listed in `validatorSet`. The three fields you edit in the
plain chain spec are:

- `session.keys` — one entry per initial validator:
  `[account, account, { "aura": <sr25519 pub>, "grandpa": <ed25519 pub> }]`.
- `validatorSet.initialValidators` — the list of initial validator accounts
  (must match the accounts in `session.keys`).
- `sudo.key` — the account allowed to call `validatorSet.add_validator` /
  `remove_validator`. Guard this key carefully.

For each initial validator:

1. Generate an Aura (sr25519) and a GRANDPA (ed25519) key. They can share one
   secret seed:

   ```
   ./target/release/gsy-node key generate --scheme sr25519          # account + Aura
   ./target/release/gsy-node key inspect --scheme ed25519 "<the seed>"   # GRANDPA
   ```

2. Build a plain chain spec from the preset, edit the three fields above with
   your generated public keys, then convert to a raw spec:

   ```
   ./target/release/gsy-node build-spec --chain poa --disable-default-bootnode > poa.json
   # edit session.keys / validatorSet.initialValidators / sudo.key in poa.json
   ./target/release/gsy-node build-spec --chain poa.json --raw --disable-default-bootnode > poa.raw.json
   ```

3. Distribute `poa.raw.json` to every node and insert each validator's keys into
   its keystore:

   ```
   ./target/release/gsy-node key insert --chain poa.raw.json \
     --scheme sr25519 --suri "<aura seed>" --key-type aura
   ./target/release/gsy-node key insert --chain poa.raw.json \
     --scheme ed25519 --suri "<grandpa seed>" --key-type gran
   ```

   With external keys use `--chain poa.raw.json` in place of `--chain poa` in the
   systemd units below.

## Build

```
cd gsy-node
cargo build --release --features poa
```

The `poa` feature enables both the 15 s block time and the `--chain poa` spec. A
default `cargo build --release` (what the docker compose uses) is unaffected and
keeps 6 s blocks.

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

A healthy network shows the `finalized #N` height in the status lines steadily
increasing. If `best:` keeps rising but `finalized:` is stuck, you have lost
GRANDPA quorum (see the fault-tolerance table above) — check that enough
validators are online and peered.

## Adding validators to a running network

This is the growth path: onboard a new validator **after** the initial ones are
already producing blocks. No re-genesis, no downtime for the existing nodes.

The order matters — **register the new validator's session keys before adding it
to the set**, otherwise it would be scheduled as an authority with no keys to
author/finalize with.

### 1. Provision the new host

Same as an initial validator: assign it a VPN IP, generate a stable node key,
copy the `poa.raw.json` spec and the release binary, and install the
`gsy-validator` systemd unit. Start it and let it **sync to the tip** first. It
is not yet part of the authority set, so at this stage it is effectively a full
node that happens to have `--validator` set.

### 2. Peer the new node with the existing ones

Because the network runs `--reserved-only`, peers must be listed explicitly:

- Add the new node's multiaddr
  (`/ip4/<new-vpn-ip>/tcp/30333/p2p/<new-PeerId>`) to `GSY_RESERVED_NODES` on
  each existing node, and the existing nodes' multiaddrs to the new node's env.
- Either restart the nodes to pick up the new env, or add the peer live without a
  restart via the `system_addReservedPeer` RPC (if RPC is enabled on the VPN).

Confirm the new node reports the right peer count and is at the chain tip before
continuing.

### 3. Generate and register the new validator's session keys

On the **new node** (RPC must be reachable — bind it to localhost/VPN), rotate a
fresh set of session keys into its keystore. This returns the concatenated
public session keys as a hex string:

```
curl -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"author_rotateKeys","params":[]}' \
  http://127.0.0.1:9944
# -> {"jsonrpc":"2.0","result":"0x<aura-pub><grandpa-pub>","id":1}
```

(`author_rotateKeys` is an *unsafe* RPC — only expose RPC on a trusted
interface, or use `key insert` for the `aura` and `gran` key types instead and
concatenate the two public keys in that order.)

The new validator's **account** needs a small balance to pay the transaction fee
for `set_keys`. Fund it from an endowed account (or via `sudo`) if it has none.

Then, signed by the **new validator account**, submit `session.setKeys` with the
rotated keys and an empty proof:

- `keys` = the `0x…` value returned by `author_rotateKeys`
- `proof` = `0x`

(Via Polkadot-JS Apps: *Developer → Extrinsics → session → setKeys*, or through
the JSON-RPC / a script. This binds the new validator account to the session
keys now living in its keystore.)

### 4. Add the validator to the set (sudo)

Signed by the **sudo** account, submit:

```
sudo.sudo( validatorSet.add_validator(<new validator account>) )
```

`add_validator` is root-only. It records the change immediately; the new
validator becomes an **active** Aura + GRANDPA authority at the **next session
boundary** (≤ 1 hour — see [Sessions](#sessions)).

### 5. Verify

After the next session starts, on the new node's logs you should see it begin to
author blocks (`🎁 Prepared block`, `🏆 Imported`) on its Aura slots, and the
network-wide `finalized #N` height should keep advancing with the larger set.

You can read the current set at any time from storage: `validatorSet.validators`
(the active set) and `session.validators`.

### Removing a validator

Signed by **sudo**:

```
sudo.sudo( validatorSet.remove_validator(<validator account>) )
```

It leaves the active set at the next session boundary. The runtime refuses to go
below `MinValidators` (1) to avoid stalling the chain. After it has rotated out,
you can decommission the host and drop it from the other nodes'
`GSY_RESERVED_NODES`.

> **Mind the fault-tolerance table when removing.** Dropping from 4 → 3
> validators takes you from tolerating 1 offline to tolerating 0. Remove only
> when the remaining set still meets your redundancy target.

### Limits

- The set can hold at most `MaxValidators` (32), matching the Aura/GRANDPA
  `MaxAuthorities` bound.
- New validators need their session keys registered (step 3) **before** being
  added, and their node synced and peered (steps 1–2), or they will be an
  authority that cannot produce/finalize until fixed.
