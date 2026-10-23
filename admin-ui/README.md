# admin-ui

A small, local debugging UI over [`gsy-offchain-storage`](../gsy-offchain-storage).
It reads that service's REST API and renders three drill-down views:

- **Grid** — communities and their market topology (per time-slot markets, and
  each community's asset tree).
- **Market** — per-slot trade aggregates for a community across the selected
  time window (volume, average rate, raw trades).
- **Asset** — a single asset tracked across slots: its orders, trades, traded
  energy per slot, and measurements vs forecasts.

Navigation state (time window, selected community / market / asset) lives in the
URL query string — there is no router library. This is a debugging tool, not a
production dashboard.

## Run (dev)

```bash
cd admin-ui
npm install
npm run dev
```

The dev server (Vite) proxies `/api/*` to `http://localhost:8080`, stripping the
`/api` prefix (e.g. `/api/trades` → `http://localhost:8080/trades`) — see
[`vite.config.ts`](./vite.config.ts). This keeps the browser same-origin in dev,
so no CORS is needed. It therefore requires a reachable `gsy-offchain-storage`
backend on `:8080`.

## Point at a different backend

The REST client's base URL is configurable at build/dev time via
`VITE_API_BASE_URL`:

- **Unset (default):** base is `/api`, i.e. the Vite dev proxy above.
- **Set to a full origin** (e.g. `http://some-host:8080`): requests go straight
  there. Useful when serving the built app against a remote backend. That
  backend must allow this origin (CORS); the UI does not add CORS itself.

Copy [`.env.example`](./.env.example) to `.env` (or `.env.local`) and set the
variable to override. Local `.env*` files are gitignored; `.env.example` is
tracked.

## Build / preview

```bash
npm run build      # tsc -b && vite build → dist/
npm run preview     # serve the built dist/ locally
```

## Backend-version note

`/communities` and `/trades-canonical` are newer `gsy-offchain-storage`
endpoints. Against an older backend that lacks them the UI **degrades
gracefully**:

- No `/communities` → the community list is derived client-side from `/markets`
  in the selected window (a small banner notes this).
- No `/trades-canonical` → trades fall back to `/trades`, and the resolved
  seller/buyer asset names are shown as their raw account IDs instead.

Rebuild the `gsy-offchain-storage` container to get the richer behavior (named
communities and resolved trade parties).

## Data-model gotchas

If numbers look surprising, read [`docs/plans/admin-ui.md`](../docs/plans/admin-ui.md)
§9 first. In short: a trade/order `energy_rate` is a **bid total price**
(energy × per-unit rate), not a per-kWh rate, so the UI derives per-kWh columns
itself; an asset's `area_hash` is **randomized per market/slot**, so assets are
tracked by their stable `name` and re-resolved to a hash per slot; and in the
current seed data measurement/forecast `area_uuid` values are synthetic sensor
ids that **do not join** to the topology assets' names/hashes, so those panels
legitimately render empty with an explanatory note.
