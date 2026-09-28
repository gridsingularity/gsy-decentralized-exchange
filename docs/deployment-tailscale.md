# GSY DEX — split deployment over a Tailscale mesh

Runbook for the 3-file production split:

| File | Runs on | Contains |
|------|---------|----------|
| `docker-compose.central.yml` | 1 central server | off-chain storage, matching / orchestrator / execution engines, 3-node MongoDB, a Tailscale sidecar |
| `docker-compose.node-1.yml` | validator host #1 | one `gsy-node` validator + a Tailscale sidecar |
| `docker-compose.node-2.yml` | validator host #2 | one `gsy-node` validator + a Tailscale sidecar |

Every host joins one Tailscale tailnet via a sidecar container whose network
namespace the app containers share. **No inbound/forwarded ports are required on
the validator hosts** — node↔node p2p, engine→node RPC, and node→storage all ride
the mesh. The only optionally-public port is storage `:8080` on the central host.

```
        Tailscale tailnet (100.x.y.z)
   ┌───────────────┐   RPC 9944    ┌──────────────┐
   │ central       │◀──────────────│ validator #1 │
   │  storage:8080 │──────────────▶│  gsy-node    │
   │  engines      │   storage     └──────┬───────┘
   │  mongo (local)│                p2p 30333 (mesh)
   └───────▲───────┘               ┌──────┴───────┐
           │  RPC 9944 / storage   │ validator #2 │
           └───────────────────────│  gsy-node    │
                                   └──────────────┘
```

## Prerequisites

- Docker + Docker Compose v2 on all three hosts.
- A Tailscale account and an **auth key** per host (reusable or ephemeral):
  Tailscale admin → Settings → Keys. Disable key expiry for server nodes (below).
- The three hosts do **not** need to open any ports to each other or the internet
  (Tailscale does NAT traversal / DERP relay). Only the central host optionally
  exposes `:8080` if you want off-mesh access to storage.

## 0. Pick a shared API key

Off-chain storage requires an `x-api-key` header. Choose one value used everywhere:
storage, all three engines, and — baked at **build time** — both node images.
Default is `fedecom_user`. Export it on each host before building:

```bash
export API_KEY='choose-a-strong-key'
```

## 1. Central server

```bash
export TS_AUTHKEY='tskey-auth-...'          # this host's Tailscale key
docker compose -f docker-compose.central.yml up --build -d
```

Find the central host's tailnet IP — the nodes will point their storage URL here:

```bash
docker exec tailscale-central tailscale ip -4      # e.g. 100.100.0.10
```

`GSY_NODE_HOST` (the validator RPC the engines dial) is not known yet — you'll set
it in step 2c and re-up. Until then the engines log connection retries; that's fine.

## 2. Validator nodes (repeat on host #1 and host #2)

The offchain worker bakes the storage URL + API key at **compile time**
(`option_env!`), so they are **build args**. Point `OFFCHAIN_STORAGE_URL` at the
central host's tailnet IP from step 1.

### 2a. First boot — generate this node's identity

```bash
export TS_AUTHKEY='tskey-auth-...'
export API_KEY='choose-a-strong-key'
export OFFCHAIN_STORAGE_URL='http://100.100.0.10:8080'   # central tailnet IP

mkdir -p .secrets/gsy-node-1                              # (node-2 on the other host)
docker compose -f docker-compose.node-1.yml up --build -d
```

`GSY_PUBLIC_IP` and `GSY_RESERVED_NODES` are empty in `./.env/gsy-node-1.env` on the
first boot, so the node starts standalone and creates `.secrets/gsy-node-1/node.key`
(a stable libp2p key → stable PeerId). Read both facts you need:

```bash
docker exec tailscale-node-1 tailscale ip -4                 # this node's tailnet IP
docker logs gsy-node-1 2>&1 | grep -i "Local node identity"  # this node's PeerId (12D3KooW...)
```

> Prefer explicit key generation? Instead of relying on auto-create:
> `docker run --rm -v "$PWD/.secrets/gsy-node-1:/keys" --entrypoint \`
> `/var/www/gsy-node/target/release/gsy-node <node-image> key generate-node-key --file /keys/node.key`
> (find `<node-image>` with `docker compose -f docker-compose.node-1.yml images`).

### 2b. Exchange addresses + PeerIds between the two nodes

You now have, for each node: its **tailnet IP** and its **PeerId**. Build each
node's reserved multiaddr for the *other* node:

```
/ip4/<other node tailnet ip>/tcp/30333/p2p/<other node PeerId>
```

### 2c. Set peering and restart

On **host #1**, edit `./.env/gsy-node-1.env`:

```ini
GSY_PUBLIC_IP=<node-1 tailnet ip>
GSY_RESERVED_NODES=/ip4/<node-2 tailnet ip>/tcp/30333/p2p/<node-2 PeerId>
```

On **host #2**, edit `./.env/gsy-node-2.env` symmetrically (its own IP; node-1 as the
reserved peer). Then restart each node (no rebuild needed — these are runtime env):

```bash
docker compose -f docker-compose.node-1.yml up -d
```

Finally, on the **central** host set the validator RPC target and re-up:

```bash
export GSY_NODE_HOST='<node-1 tailnet ip>'   # GSY_NODE_PORT defaults to 9944
docker compose -f docker-compose.central.yml up -d
```

## 3. Verify

```bash
# Nodes see each other as reserved peers and are producing/finalizing blocks:
docker logs gsy-node-1 2>&1 | grep -Ei "idle|finalized|peers"      # peers should be >= 1

# Storage is healthy and auth is on:
curl -f http://<central tailnet ip>:8080/health_check              # 200 (health is exempt)
curl -s -o /dev/null -w '%{http_code}\n' http://<central>:8080/orders          # 401 (no key)
curl -s -o /dev/null -w '%{http_code}\n' -H "x-api-key: $API_KEY" http://<central>:8080/orders  # 200

# Storage reaches mongo through the shared namespace (should print an address):
docker exec gsy-offchain-storage getent hosts mongo1
```

## Configuration reference

Node runtime (`./.env/gsy-node-{1,2}.env`, read by the container):

| Var | Meaning |
|-----|---------|
| `GSY_CHAIN` | `poa` (built-in Alice/Bob dev keys) or an absolute path to a raw spec |
| `GSY_VALIDATOR` | `--alice` / `--bob` (dev), or `--validator --name <n>` for real keys |
| `GSY_PUBLIC_IP` | this node's Tailscale IP (advertised as `--public-addr`) |
| `GSY_RESERVED_NODES` | the other validator(s) as tailnet multiaddr(s) |

Node build-time (shell env → image build args; **rebuild to change**):

| Var | Meaning |
|-----|---------|
| `OFFCHAIN_STORAGE_URL` | central storage URL the offchain worker POSTs to |
| `API_KEY` | `x-api-key` the offchain worker sends (must match storage) |
| `TS_AUTHKEY` | Tailscale auth key for the sidecar |

Central (shell env):

| Var | Meaning |
|-----|---------|
| `TS_AUTHKEY` | Tailscale auth key for `tailscale-central` |
| `GSY_NODE_HOST` / `GSY_NODE_PORT` | a validator's tailnet RPC address (default port 9944) |
| `API_KEY` | shared storage key (server + engines) |

## Going to production

- **Real validator keys:** build a raw chain spec with your own aura/grandpa
  authorities, set `GSY_CHAIN=/abs/path/spec.raw.json` and
  `GSY_VALIDATOR="--validator --name <n>"`, and insert each node's session keys
  into its keystore (`author_insertKey`). The `poa`/`--alice`/`--bob` default is
  for testing only.
- **Mesh-only storage:** delete the `ports: - "8080:8080"` mapping from
  `tailscale-central` in `docker-compose.central.yml` so storage is reachable only
  over the tailnet (the `API_KEY` then becomes defense-in-depth).
- **Tailnet stability:** disable key expiry on the three server machines in the
  Tailscale admin console (or use `--authkey` keys with expiry disabled) so a host
  never silently drops off the mesh. Optionally lock things down with Tailscale ACLs
  so only these hosts can reach `30333`/`9944`/`8080`.
- **MongoDB** is internal to the central docker bridge (no host ports). Add a
  `ports:` mapping to `mongo1` only if you need host access.

## Troubleshooting

- **Sidecar won't start / no tailnet IP:** the sidecar needs kernel networking —
  confirm `/dev/net/tun` exists on the host and the container has `NET_ADMIN`
  (`docker logs tailscale-node-1`). Check the auth key is valid and unexpired.
- **Engines/storage can't resolve `mongo1`:** they resolve it through the shared
  `tailscale-central` namespace via docker's embedded DNS. Verify with
  `docker exec gsy-offchain-storage getent hosts mongo1`; ensure mongo and the
  sidecar are on the same (default) compose network.
- **Nodes don't peer:** re-check each `GSY_RESERVED_NODES` multiaddr (right tailnet
  IP + PeerId), that both nodes are up on the tailnet (`tailscale status`), and that
  `--reserved-only` peers can actually reach each other (`tailscale ping <ip>`).
- **Worker POSTs 401 to storage:** the node image's build-time `API_KEY` must equal
  the storage `API_KEY`; rebuild the node image after changing it (it's compile-time).
- **Changed storage URL/key for a node but nothing happened:** those are baked at
  build time — rebuild the node image (`up --build`).
