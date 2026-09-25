# GSY DEX Analytics Engine

The analytics engine calculates the GSY DEX KPIs. On a fixed interval (15 minutes by default) it reads trades and meter readings directly from the off-chain storage MongoDB, computes the KPIs per community and 15-minute energy delivery slot, and stores the results in the `kpi_results` collection. A read-only HTTP API serves the stored results.

The only KPI in this version is the **procurement cost per kWh**.

## How it works

Every tick the engine recomputes the last `ANALYTICS_LOOKBACK_HOURS` of slots that ended at least `ANALYTICS_SETTLEMENT_DELAY_MINUTES` ago. Recomputing a rolling window picks up late trades and readings. Results are upserted by `(kpi_id, community_id, granularity, period_start)`, so rerunning a window never creates duplicates.

It reads these off-chain storage collections and never writes to them:

| Collection | Used for |
|---|---|
| `communities` | Rebuilding each community's market ids, to attribute trades to communities. |
| `trades` | P2P energy bought and its clearing price. |
| `measurement_points`, `timeseries` | Net metered energy per facility and slot (positive = import, negative = export). |
| `facilities` | Mapping each metered facility to its owner, whose on-chain id is the trade's buyer. |

Trades on markets that match no known community, measurement points that are not per-slot kWh values, and readings whose facility has no owner are left out and counted in the tick log.

## Procurement cost per kWh

The average cost a community pays for each kWh it consumes, with P2P trading in place (formula `p2p_pricing`), per community and 15-minute slot:

```
value = (P2P cost + residual grid energy × tariff) / member net demand
```

- **P2P cost**: sum of `energy × clearing price` over the community's trades in the slot (rejected trades excluded).
- **Member net demand**: sum over facilities of `max(0, net metered energy)`. A buyer without a meter reading counts its bought energy as demand.
- **Residual grid energy**: per facility, the demand not covered by P2P purchases, `max(0, demand − bought)`.
- **Tariff**: the grid tariff in EUR/kWh from `ANALYTICS_GRID_TARIFF_EUR_PER_KWH`, or the community's entry in `ANALYTICS_GRID_TARIFF_OVERRIDES`.

The base case buys the whole net demand from the grid: `baseline_value = tariff`, and `improvement_pct = (baseline_value − value) / baseline_value × 100`.

`value` is `null` with a `null_reason` when net demand is zero (`zero_net_demand`) or no tariff is configured (`missing_tariff`). Every input sum is stored in `components`, so a result can be checked or recomputed.

Example: meter readings A +4 kWh, B +2 kWh, P −5 kWh; trades P→A 3 kWh at 0.12 and P→B 1.5 kWh at 0.15; tariff 0.30. P2P cost 0.585 €, demand 6 kWh, residual 1.5 kWh (0.45 €), so the value is (0.585 + 0.45) / 6 = **0.1725 €/kWh**, 42.5% below the 0.30 base case.

Penalties are not included yet; they will be added once the execution engine persists them.

## Configuration

All settings are environment variables.

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL_SCHEME` | `mongodb` | Same variables as off-chain storage. |
| `DATABASE_HOST` | `mongodb` | |
| `DATABASE_USERNAME` | `gsy` | |
| `DATABASE_PASSWORD` | `gsy` | Hidden in the logged config. |
| `DATABASE_NAME` | `offchain_storage` | Database read from. |
| `ANALYTICS_RESULTS_DATABASE_NAME` | `DATABASE_NAME` | Database the results are written to. |
| `ANALYTICS_RESULTS_COLLECTION` | `kpi_results` | |
| `ANALYTICS_INTERVAL_SECONDS` | `900` | Time between ticks. The first tick runs at startup. |
| `ANALYTICS_LOOKBACK_HOURS` | `48` | Window recomputed on every tick. |
| `ANALYTICS_SETTLEMENT_DELAY_MINUTES` | `15` | Only slots that ended at least this long ago are computed. |
| `ANALYTICS_BACKFILL_FROM` | unset | Unix seconds or RFC 3339. At startup, computes everything from this time up to the tick window, one day at a time. |
| `ANALYTICS_ENABLED_KPIS` | `procurement_cost_per_kwh` | Comma-separated KPI ids. Unknown ids stop the service at startup. |
| `ANALYTICS_GRANULARITIES` | `15min` | Only `15min` is supported. |
| `ANALYTICS_GRID_TARIFF_EUR_PER_KWH` | unset | Grid tariff for all communities. Without any tariff, values and baselines are `null`. |
| `ANALYTICS_GRID_TARIFF_OVERRIDES` | unset | Per-community tariffs, e.g. `Pilot1=0.28,Pilot2=0.25`. |
| `ANALYTICS_API_HOST` | `0.0.0.0` | HTTP API bind address. |
| `ANALYTICS_API_PORT` | `8081` | HTTP API port. |
| `TIME_SLOT_SEC` | `900` | Slot length, shared with the other services. |
| `RUST_LOG` | `info` | |

Empty values count as unset, so compose files can pass `${VAR:-}`.

## Running

Locally, against a MongoDB on `localhost`:

```bash
export DATABASE_HOST=localhost:27017
export ANALYTICS_GRID_TARIFF_EUR_PER_KWH=0.30
cargo run -p gsy-analytics-engine
```

With Docker Compose, from the repository root (the service starts after `mongodb` and `gsy-offchain-storage`):

```bash
docker compose up --build gsy-analytics-engine
```

Set `ANALYTICS_GRID_TARIFF_EUR_PER_KWH` (and optionally the other `ANALYTICS_*` variables) in the shell or `.env` before starting it. Each tick logs one `KPI tick finished` line with the window, the number of trades and readings, the skipped data and the results written.

## API

| Method | Path | Returns |
|---|---|---|
| GET | `/health_check` | `200 OK` |
| GET | `/kpis/procurement-cost-per-kwh` | JSON array of procurement cost per kWh results |

Query parameters, all optional:

| Parameter | Description |
|---|---|
| `start_time` | Unix seconds. Periods with `period_start >= start_time`. |
| `end_time` | Unix seconds. Periods with `period_start < end_time`. |
| `community_id` | Only this community. |

Results are ordered by community and period start. `end_time` at or before `start_time`, or a non-numeric time, returns `400`.

```bash
curl 'http://localhost:8081/kpis/procurement-cost-per-kwh?start_time=1758621600&end_time=1758625200&community_id=Pilot1'
```

```json
[
  {
    "kpi_id": "procurement_cost_per_kwh",
    "community_id": "Pilot1",
    "granularity": "15min",
    "period_start": 1758621600,
    "period_end": 1758622500,
    "value": 0.1725,
    "null_reason": null,
    "unit": "EUR/kWh",
    "components": {
      "p2p_cost": 0.585,
      "p2p_energy_kwh": 4.5,
      "net_demand_kwh": 6.0,
      "grid_residual_energy_kwh": 1.5,
      "grid_residual_cost": 0.45,
      "community_net_import_kwh": 1.0,
      "tariff_eur_per_kwh": 0.3,
      "baseline_cost": 1.8,
      "trade_count": 2,
      "facility_count": 3,
      "facilities_with_reading": 3,
      "buyers_without_reading": 0
    },
    "baseline": { "baseline_value": 0.3, "improvement_pct": 42.5 },
    "computed_at": 1758624000,
    "engine_version": "0.1.0"
  }
]
```

The response types are `primitives::db_api_schema::kpi::ProcurementCostResultSchema`, so Rust clients can deserialize them directly. The same documents can be queried in MongoDB:

```js
db.kpi_results.find({kpi_id: "procurement_cost_per_kwh", granularity: "15min"}).sort({period_start: -1})
```

## Tests

Tests live under `tests/`. The KPI maths, mapping, periods, config and registry tests need nothing else:

```bash
cargo test -p gsy-analytics-engine --test procurement_cost_per_kwh --test mapping --test period --test config --test registry --test db --test kpi_schema
```

`tests/mongo_integration.rs` and `tests/api.rs` need a MongoDB, configured through the `DATABASE_*` variables. Each test uses its own randomly named database and drops it afterwards:

```bash
DATABASE_HOST=localhost:27017 cargo test -p gsy-analytics-engine
```

In CI they run through `./run_integration_tests.sh` as `gsy-analytics-engine-integration-test`.

## Adding a KPI

1. Add its result `components` type to `primitives/src/db_api_schema/kpi.rs`.
2. Implement the `Kpi` trait in a new file under `src/kpi/`, as pure functions over the loaded `Dataset`, and declare the data it needs in `requirements()`.
3. Add a `KpiResult` variant and register the KPI in `build_registry`.
4. Add a GET handler under `src/api/` and its route in `run_http_server`.

The loaders, the result writer and the scheduler do not change.
