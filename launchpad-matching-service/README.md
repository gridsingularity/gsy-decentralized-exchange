# Launchpad Matching Service

A standalone version for the GSY DEX matching service built in Rust. It implements the **Pay-As-Bid** algorithm to match energy buy (bid) and sell (offer) orders, persists trade data in MongoDB, and exposes a REST API secured with JWT authentication.

This service is part of the [Grid Singularity (GSY)](https://gridsingularity.com/) decentralized exchange for peer-to-peer energy and flexibility trading.

---

## Table of Contents

- [Overview](#overview)
- [Tech Stack](#tech-stack)
- [Project Structure](#project-structure)
- [Getting Started](#getting-started)
  - [Prerequisites](#prerequisites)
  - [Configuration](#configuration)
  - [Running Locally](#running-locally)
  - [Running with Docker](#running-with-docker)
- [Authentication](#authentication)
- [API Reference](#api-reference)
  - [Health Check](#get-health-check)
  - [Readiness Check](#get-ready)
  - [Errors](#errors)
  - [Obtain Token](#post-token)
  - [Match Orders](#post-match)
  - [Filter Matches](#get-matches)
  - [Market Statistics](#get-statistics)
  - [Get Markets](#get-markets)
- [Matching Algorithm](#matching-algorithm)
  - [Performance](#performance)
- [Data Models](#data-models)
- [Running Tests](#running-tests)

---

## Overview

The Launchpad Matching Service is responsible for:

1. **Order Matching**: accepts lists of open bids and offers and matches them using the Pay-As-Bid algorithm.
2. **Trade Storage**: persists all matches and market statistics in MongoDB.
3. **Market Analytics**: provides time-series statistics on traded energy, average trade rates, and market success rates with configurable aggregation resolutions (per slot, quarter-hour, daily, calendar month) in a chosen time zone.
4. **Market Discovery**: exposes an endpoint to list unique market IDs associated with a user.

---

## Tech Stack

| Component      | Technology                        |
|----------------|-----------------------------------|
| Language       | Rust 2024 edition                 |
| Web framework  | Actix-web 4.5.1                   |
| Async runtime  | Tokio                             |
| Database       | MongoDB (driver v2.8.2)           |
| Authentication | JWT (HS256) + bcrypt passwords    |
| Reverse proxy  | Caddy (with rate limiting)        |
| Container      | Docker                            |

**Key dependencies:**

| Crate                   | Purpose                          |
|-------------------------|----------------------------------|
| `actix-web`             | HTTP server and routing          |
| `mongodb`               | Database driver                  |
| `jsonwebtoken`          | JWT generation and validation    |
| `bcrypt`                | Password hashing                 |
| `serde` / `serde_json`  | Serialization                    |
| `tokio`                 | Async runtime                    |
| `config`                | Configuration management         |
| `gsy-offchain-primitives` | Order schemas and shared types |

---

## Project Structure

```
launchpad-matching-service/
├── src/
│   ├── main.rs                  # Entry point; creates the shared MongoDB client
│   ├── lib.rs                   # Library root
│   ├── configuration.rs         # Configuration loading
│   ├── bin/
│   │   └── create-service-user.rs # CLI to create or rotate the service account
│   ├── api/
│   │   ├── mod.rs               # API module definition
│   │   ├── views.rs             # HTTP handlers (routes)
│   │   ├── controller.rs        # Business logic and matching
│   │   ├── model.rs             # MongoDB query layer
│   │   └── types.rs             # Domain type definitions
│   └── auth/
│       ├── mod.rs               # Auth module definition
│       ├── jwt.rs               # JWT creation and validation
│       ├── views.rs             # /token endpoint handler
│       └── model.rs             # User model and DB operations
├── tests/
│   ├── common/                  # Pre-MS-10 matching (reference), seeded RNG, payload builder
│   ├── auth_model.rs            # Password hashing and user upsert tests
│   ├── bench_matching.rs        # Matching benchmark (ignored by default)
│   ├── body_limit.rs            # JSON body limit tests
│   ├── configuration.rs         # Configuration tests
│   ├── controller.rs            # Matching algorithm tests
│   ├── matching_golden.rs       # Optimized vs reference matching (golden test)
│   ├── model.rs                 # Database layer tests
│   └── views.rs                 # API integration tests
├── configuration.yaml           # Default configuration
├── openapi.yaml                 # OpenAPI 3.0 specification
├── Caddyfile                    # Caddy reverse proxy config
├── Dockerfile.matching-service  # Service Docker image
├── Dockerfile.caddy             # Caddy Docker image
└── Cargo.toml                   # Rust project manifest
```

---

## Getting Started

### Prerequisites

- Rust (2024 edition or later): [rustup.rs](https://rustup.rs/)
- MongoDB instance (local or remote)
- (Optional) Docker and Docker Compose for containerised deployment

### Configuration

Configuration is loaded from `configuration.yaml`. All values can be overridden with environment variables (uppercase with underscores, e.g. `DATABASE_HOST`).

| Setting              | Default       | Description                              |
|----------------------|---------------|------------------------------------------|
| `application_host`   | `0.0.0.0`     | HTTP server bind address                 |
| `application_port`   | `9876`        | HTTP server port                         |
| `database_host`      | `localhost`   | MongoDB hostname                         |
| `database_username`  | `gsy`         | MongoDB username                         |
| `database_password`  | `gsy`         | MongoDB password                         |
| `database_name`      | `launchpad`   | MongoDB database name                    |
| `database_url_scheme`| `mongodb`     | MongoDB connection scheme                |
| `jwt_secret`         | `test_secret` | Secret key for signing JWT tokens        |
| `json_body_limit`    | `16777216`    | Maximum JSON request body in bytes (16 MiB), must be > 0; larger bodies get `413` |

> **Important:** Change `jwt_secret` (and database credentials) before running in any non-development environment.

Example `configuration.yaml`:

```yaml
application_host: "0.0.0.0"
application_port: "9876"
database_host: "localhost"
database_username: "gsy"
database_password: "gsy"
database_name: "launchpad"
database_url_scheme: "mongodb"
jwt_secret: "my-secure-secret"
json_body_limit: "16777216"
```

`json_body_limit` (environment variable `JSON_BODY_LIMIT`) applies to every JSON body (`POST /match`, `POST /token`). The default, 16 MiB, is above the largest launchpad plan request size (Advanced, 10 MiB) because the launchpad backend adds a `user_id` to every order before forwarding a batch. The service refuses to start if it is `0` or not a number.

### Running Locally

```bash
# Clone the repository
git clone <repo-url>
cd launchpad-matching-service

# Build
cargo build --release

# Run (ensure MongoDB is accessible)
cargo run --release
```

The service will start on `http://0.0.0.0:9876` by default.

At startup the service creates its MongoDB indexes if they are missing (see [Indexes](#indexes)). If that fails, the error is logged to stderr and the service starts anyway.

### Running with Docker

Two Docker images are provided:

- **`Dockerfile.matching-service`**: builds and runs the Rust service.
- **`Dockerfile.caddy`**: builds a Caddy reverse proxy that forwards traffic to the service on port 9876, with two rate-limit zones per client IP: a general one (100 req/s) and a strict one for `/token` (5 requests per minute). No CORS headers are sent, since the service is only called server to server.

Request body size and Caddy: Caddy's `reverse_proxy` does not limit request bodies by default, so the Caddyfile needs no `request_body` setting; the service itself rejects bodies over `json_body_limit` with `413`. If a limit at the proxy is wanted, add `request_body { max_size 16MiB }` to the site block (at least `json_body_limit`; note that `16MB` means 16,000,000 bytes).

A Docker Compose file is available for cluster deployment:

```bash
docker compose up
```

---

## Authentication

All endpoints except `/health-check`, `/ready` and `/token` require a **Bearer JWT token** in the `Authorization` header.

**Header format:**
```
Authorization: Bearer <token>
```

**Token properties:**
- Algorithm: HS256
- Expiration: 24 hours
- Claims: `sub` (username), `iat` (issued-at), `exp` (expiration)

**Typical flow:**
1. `POST /token` with valid credentials to receive an `access_token`.
2. Include the token in the `Authorization` header for all subsequent requests.

User accounts are stored in the `users` MongoDB collection with bcrypt-hashed passwords.

### Service account

The service is meant to be called by the launchpad backend only, using a single service account. Create the account, or rotate its password, with the `create-service-user` CLI. It reads the password from the first line of stdin (never from arguments or environment variables), requires at least 16 characters, stores a bcrypt hash, and replaces the existing hash if the username already exists.

Generate a strong password, or enter one without echoing it:

```bash
PW="$(openssl rand -base64 32)"
# or: read -rs PW
```

Locally (uses `configuration.yaml` or environment variables for MongoDB):

```bash
printf '%s\n' "$PW" | cargo run --bin create-service-user -- launchpad-backend
```

In Docker:

```bash
printf '%s\n' "$PW" | docker compose -f docker-compose.launchpad-matching-service.yml exec -T matching-service \
  /var/www/launchpad-matching-service/target/release/create-service-user launchpad-backend
```

Then store the same password in the launchpad backend configuration and `unset PW`.

---

## API Reference

### `GET /health-check`

Liveness check: returns `200 OK` if the service process is running. It does not contact MongoDB, so a `200` here does not mean the service can serve requests; use `/ready` for that. No authentication required.

---

### `GET /ready`

Readiness check: pings MongoDB through the shared client. No authentication required.

- `200 OK` if the ping succeeds within 5 seconds.
- `503 Service Unavailable` if MongoDB returns an error or does not answer within 5 seconds:

```json
{
  "error": "database_unavailable",
  "message": "The database is not reachable."
}
```

---

### Errors

If a MongoDB operation fails, `/match`, `/matches`, `/statistics` and `/markets` return `500 Internal Server Error` with this body instead of a `200` with empty data:

```json
{
  "error": "internal_error",
  "message": "A database operation failed."
}
```

The underlying error is written to the service log (stderr) and is not returned to the caller. `/token` returns the same body when the user lookup fails. Authentication failures return `401` as described in [Authentication](#authentication).

A malformed JSON body (`/match`, `/token`) or query string (`/match`, `/matches`, `/statistics`, `/markets`), for example a missing field, a wrong type, an unknown `resolution` / `sort` value or a `dry_run` other than `true` / `false`, returns `400 Bad Request` with the serde error text:

```json
{
  "error": "invalid_request",
  "message": "missing field `user_id` at line 1 column 14"
}
```

A body larger than `json_body_limit` (default 16 MiB, see [Configuration](#configuration)) returns `413 Payload Too Large` in the same shape, before the body is parsed:

```json
{
  "error": "payload_too_large",
  "message": "JSON payload (16777217 bytes) is larger than allowed (limit: 16777216 bytes)."
}
```

`/statistics` returns `400` with this body when `tz` is not a time zone MongoDB knows (see [Market Statistics](#get-statistics)):

```json
{
  "error": "invalid_timezone",
  "message": "Unknown time zone."
}
```

---

### `POST /token`

Obtain a JWT access token.

**Request body:**
```json
{
  "username": "alice",
  "password": "secret"
}
```

**Response `200`:**
```json
{
  "access_token": "<jwt>",
  "token_type": "bearer"
}
```

**Error codes:** `401` (invalid credentials), `500` (server error)

---

### `POST /match`

Submit bids and offers to be matched using the Pay-As-Bid algorithm. Resulting matches are persisted in MongoDB and market statistics are updated atomically, unless `dry_run=true` is given.

Orders are matched per (`market_id`, `time_slot`). The response contains the matches of every time slot in the batch, and a key for every market in the batch (an empty array if it had no match). Within a market, matches are ordered by `time_slot` ascending.

The body may be up to `json_body_limit` bytes (default 16 MiB); larger bodies return `413` (see [Errors](#errors)). A 10 MiB batch (about 29,000 orders) is matched in tens of milliseconds, see [Performance](#performance).

**Authentication:** Required

**Query parameters:**

| Parameter | Required | Type    | Description |
|-----------|----------|---------|-------------|
| `dry_run` | No       | Boolean | `true` or `false` (default `false`). With `true` the matches are computed and returned exactly as in a normal run, but nothing is stored: no match is inserted into `matches` and `market_data` is not updated. The `match_id` values in a dry-run response are freshly generated and not persisted, so they cannot be looked up later and differ from the ids a later normal run of the same batch returns. Any other value returns `400` (see [Errors](#errors)). |

Example: `POST /match?dry_run=true` with the request body below.

**Request body:**
```json
{
  "user_id": "user-123",
  "orders": [
    {
      "_id": "order-abc",
      "status": "Open",
      "order": {
        "type": "Bid",
        "data": {
          "buyer": "buyer-address",
          "nonce": 1,
          "bid_component": {
            "area_uuid": "area-1",
            "market_id": "market-xyz",
            "time_slot": 1700000000,
            "creation_time": 1699999900,
            "energy": 10.0,
            "energy_rate": 30.0
          }
        }
      }
    }
  ]
}
```

`status` values: `Open`, `Executed`, `Expired`, `Deleted`
`type` values: `Bid`, `Offer`

**Response `200`:** A map of `market_id` to arrays of match objects:

```json
{
  "market-xyz": [
    {
      "match_id": "6717a8c2e4b0f1a2b3c4d5e6",
      "user_id": "user-123",
      "market_id": "market-xyz",
      "time_slot": 1700000000,
      "bid_order_id": "order-abc",
      "offer_order_id": "order-def",
      "bid": { "buyer": "buyer-address", "nonce": 1, "bid_component": { ... } },
      "offer": { "seller": "seller-address", "nonce": 1, "offer_component": { ... } },
      "residual_bid": null,
      "residual_offer": null,
      "selected_energy": 10.0,
      "energy_rate": 30.0
    }
  ]
}
```

- `match_id`: unique id of the match, a 24-character hex MongoDB ObjectId assigned when the match is created (time-ordered).
- `bid_order_id` / `offer_order_id`: the `_id` of the submitted order the bid / offer came from, so callers can map matches back to their orders.
- `residual_bid` / `residual_offer` are non-null when an order was only partially filled, containing the remaining quantity.

---

### `GET /matches`

Retrieve previously recorded matches with optional filtering.

**Authentication:** Required

**Query parameters:**

| Parameter    | Required | Type    | Description                       |
|--------------|----------|---------|-----------------------------------|
| `user_id`    | Yes      | String  | Filter by user                    |
| `market_id`  | No       | String  | Filter by market                  |
| `start_time` | Yes      | Integer | Unix timestamp (inclusive lower bound) |
| `end_time`   | Yes      | Integer | Unix timestamp (inclusive upper bound) |
| `limit`      | No       | Integer | Maximum number of results         |
| `offset`     | No       | Integer | Number of matches to skip (paging) |
| `sort`       | No       | String  | `asc` (default) or `desc`         |

**Response `200`:** Array of match objects (see [Match Orders](#post-match)), sorted by `time_slot` and then `match_id`, both in the `sort` direction. The tie-break on `match_id` makes `offset` / `limit` pages stable: consecutive pages do not overlap and together cover the whole result. The total for a page count is `total_matches` of [`/statistics`](#get-statistics) with the same filters.

---

### `GET /statistics`

Retrieve market statistics and time-series data for a user.

**Authentication:** Required

**Query parameters:**

| Parameter    | Required | Type    | Description                                      |
|--------------|----------|---------|--------------------------------------------------|
| `user_id`    | Yes      | String  | Filter by user                                   |
| `market_id`  | No       | String  | Filter by market (omit to aggregate all markets) |
| `start_time` | Yes      | Integer | Unix timestamp                                   |
| `end_time`   | Yes      | Integer | Unix timestamp                                   |
| `resolution` | No       | String  | `no_aggregation` (default), `quarter_hour`, `day`, `month` |
| `tz`         | No       | String  | IANA time zone for the buckets, e.g. `Europe/Zurich` (default `UTC`) |

`start_time` and `end_time` are inclusive bounds on `time_slot`.

**Resolution values:**
- `no_aggregation`: one data point per time slot (`tz` is not used).
- `quarter_hour`: 15-minute buckets starting at :00, :15, :30 and :45 local time.
- `day`: calendar days starting at local midnight in `tz` (in `Europe/Zurich` that is 23:00 UTC in winter and 22:00 UTC in summer).
- `month`: calendar months starting at local midnight of the 1st in `tz`. **Changed:** previously these were fixed 30-day buckets counted from the Unix epoch.

Bucketing is done by MongoDB (`$dateTrunc`, MongoDB 5.0 or later). The `time_slot` of a point is the start of its bucket in Unix seconds (UTC). Series are sorted by `time_slot` ascending.

`tz` must be 1 to 64 characters of `A-Z a-z 0-9 _ + - /`; otherwise, or if MongoDB does not know the zone (error code 40485; names are case-sensitive, so `utc` is rejected), the response is `400` with `"error": "invalid_timezone"` (see [Errors](#errors)). Any other database error stays `500`. MongoDB only checks the zone when it is used, so with `resolution=no_aggregation` a well-formed but unknown zone is ignored.

**Response `200`:**
```json
{
  "average_trade_rate_timeseries": [
    { "time_slot": 1700000000, "average_energy_rate": 27.5 }
  ],
  "energy_timeseries": [
    { "time_slot": 1700000000, "matched_energy_kWh": 50.0, "unmatched_energy_kWh": 10.0 }
  ],
  "total_matches": 12,
  "success_rate": 0.833,
  "active_traders": 7,
  "markets": 2
}
```

- `average_trade_rate_timeseries`: mean `energy_rate` over all trades in the bucket. For `quarter_hour`, `day` and `month` this is a simple mean over trades; previously `day` and `month` averaged the per-slot means, which weighted a slot with one trade like a slot with many.
- `energy_timeseries`: matched and unmatched energy summed per bucket (from `market_data`).
- `total_matches`: matches in the window (from `market_data`).
- `active_traders`: number of distinct accounts among `bid.buyer` and `offer.seller` of the matches in the window (an account that both bought and sold counts once).
- `markets`: number of distinct `market_id` values in `market_data` in the window.

`success_rate` = `matched_energy / (matched_energy + unmatched_energy)` over the window, `0` when there is no data.

---

### `GET /markets`

List all unique market IDs associated with a user.

**Authentication:** Required

**Query parameters:**

| Parameter | Required | Type   | Description    |
|-----------|----------|--------|----------------|
| `user_id` | Yes      | String | Filter by user |

**Response `200`:** Array of market ID strings.

```json
["market-xyz", "market-abc"]
```

---

## Matching Algorithm

The service implements the **Pay-As-Bid** algorithm:

### Sorting

- **Bids** are sorted in **descending** order by `energy_rate` (highest willingness-to-pay first).
- **Offers** are sorted in **ascending** order by `energy_rate` (lowest willingness-to-sell first).

### Matching Rules

A bid and offer are eligible to match when:
1. `offer_energy_rate <= bid_energy_rate`: the offer is priced at or below the bid.
2. `bid.area_uuid != offer.area_uuid`: parties must be from different grid areas.
3. Both orders still have remaining energy > ε (0.000001 kWh).

Both sorts are stable: orders with the same `energy_rate` keep their order in the request.

### Execution

- The batch is grouped once by (`market_id`, `time_slot`); each group is matched on its own.
- The algorithm iterates through sorted offers (outer loop) and, for each offer, iterates through sorted bids (inner loop).
- `selected_energy = min(remaining_bid_energy, remaining_offer_energy)`
- The **matched energy rate equals the bid's energy rate** (pay-as-bid principle).
- Available energy is tracked per submitted order and decremented after each partial or full match. Two orders with identical content but different `_id`s are separate orders (before MS-10 they shared their remaining energy, which was a bug).
- Exhausted orders (less than ε left) are skipped without further work. Because bids are sorted by descending rate, the inner loop stops at the first bid priced below the offer, and the outer loop stops at the first offer priced above every remaining bid. These shortcuts give the same matches, in the same order and with bit-identical energies, as visiting every pair (checked by `tests/matching_golden.rs` against the pre-MS-10 implementation in `tests/common/reference.rs`).

### Residual Orders

When an order is only partially matched, a **residual order** is created with:
- Energy = original energy − matched energy
- Nonce incremented by 1

Residuals are stored alongside the match record.

### Performance

`tests/bench_matching.rs` measures the matching alone and a full in-process `POST /match?dry_run=true` (JSON parsing, matching and JSON response, no network) at the plan maximum request sizes, before (`tests/common/reference.rs`) and after MS-10:

```bash
cargo test --release --test bench_matching -- --ignored --nocapture
# skip the old implementation above a body size (its 10 MiB worst case takes minutes)
BENCH_SKIP_OLD_ABOVE=600000 cargo test --release --test bench_matching -- --ignored --nocapture
```

Shapes: realistic (96 quarter-hour slots, about 20 orders per market and slot, a new market every 1,920 orders), worst case (every order in one market and one slot) and single area (one market, slot and area, so nothing matches and every price-compatible pair is visited: the worst case of the optimized algorithm). Release build, one machine (Apple Silicon laptop), median of up to 10 runs:

| Shape | Body | Orders | Markets x slots | Largest group (bids x offers) | Matches | Matching, old | Matching, new | `POST /match`, old | `POST /match`, new |
|---|---|---|---|---|---|---|---|---|---|
| realistic | 50 KiB | 141 | 1 x 96 | 1 x 1 | 23 | 0.36 ms | 0.10 ms | 0.35 ms | 0.22 ms |
| realistic | 500 KiB | 1,417 | 1 x 96 | 8 x 7 | 825 | 6.2 ms | 0.76 ms | 6.6 ms | 1.8 ms |
| realistic | 10 MiB | 29,039 | 16 x 96 | 10 x 10 | 17,659 | 453 ms | 12 ms | 478 ms | 40 ms |
| worst case | 50 KiB | 141 | 1 x 1 | 69 x 72 | 102 | 4.4 ms | 0.06 ms | 4.6 ms | 0.21 ms |
| worst case | 500 KiB | 1,417 | 1 x 1 | 718 x 699 | 940 | 396 ms | 0.48 ms | 399 ms | 1.9 ms |
| worst case | 10 MiB | 29,040 | 1 x 1 | 14,567 x 14,473 | 19,291 | 183 s | 11 ms | 168 s | 41 ms |
| single area | 50 KiB | 141 | 1 x 1 | 75 x 66 | 0 | 0.04 ms | 0.02 ms | 0.10 ms | 0.09 ms |
| single area | 500 KiB | 1,417 | 1 x 1 | 687 x 730 | 0 | 1.2 ms | 0.39 ms | 1.7 ms | 1.1 ms |
| single area | 10 MiB | 29,039 | 1 x 1 | 14,494 x 14,545 | 0 | 479 ms | 107 ms | 505 ms | 112 ms |

The orders of the benchmark are about 360 bytes of JSON each (UUID market and area ids, 48-character accounts), so a 10 MiB body holds about 29,000 orders. Dry runs only: storing the matches (a normal run) adds the MongoDB inserts, which are not measured here.

The matching cost after MS-10 grows with the number of orders and matches, not with bids x offers, except for pairs in the same area, which are visited and skipped. Most of the request time at 10 MiB is JSON parsing and serializing the response.

---

## Data Models

### MongoDB Collections

#### `users`
```
{
  username: String,
  password_hash: String   // bcrypt
}
```

#### `matches`
```
{
  match_id: String,       // hex ObjectId, unique
  user_id: String,
  market_id: String,
  time_slot: u64,         // Unix timestamp
  bid_order_id: String,   // _id of the submitted bid order
  offer_order_id: String, // _id of the submitted offer order
  bid: {
    buyer: String,
    nonce: u32,
    bid_component: { area_uuid, market_id, time_slot, creation_time, energy, energy_rate }
  },
  offer: {
    seller: String,
    nonce: u32,
    offer_component: { area_uuid, market_id, time_slot, creation_time, energy, energy_rate }
  },
  residual_bid: <bid> | null,
  residual_offer: <offer> | null,
  selected_energy: f64,
  energy_rate: f64
}
```

#### `market_data`
```
{
  user_id: String,
  market_id: String,
  time_slot: u64,
  submitted_bid_count: u64,
  submitted_offer_count: u64,
  total_matches: u64,
  total_matched_energy_kWh: f64,
  total_unmatched_energy_kWh: f64
}
```

`market_data` documents are upserted using MongoDB `$inc` operators, making incremental updates atomic.

Documents written before `match_id`, `bid_order_id` and `offer_order_id` were added are not migrated; they cannot be read by `/matches` and are expected to be dropped on deploy.

### Indexes

Created at startup if missing (idempotent, one `createIndexes` call per index so one failure does not block the others):

| Collection    | Keys                                       | Unique |
|---------------|--------------------------------------------|--------|
| `matches`     | `{ user_id: 1, time_slot: 1, match_id: 1 }` (serves the `/matches` sort) | No |
| `matches`     | `{ user_id: 1, market_id: 1, time_slot: 1 }` | No   |
| `matches`     | `{ match_id: 1 }`                          | Yes    |
| `market_data` | `{ user_id: 1, market_id: 1, time_slot: 1 }` (the upsert key) | Yes |
| `market_data` | `{ user_id: 1, time_slot: 1 }`             | No     |

If an index cannot be created (for example a unique index over old documents with duplicates, or an existing index with the same keys but different options), the error is logged to stderr and the service keeps running without it.

After creating these, startup drops the obsolete `matches` index `user_id_1_time_slot_1` (`{ user_id: 1, time_slot: 1 }`) of earlier versions, now replaced by the index above. If it does not exist, nothing happens; other failures are logged the same way.

---

## Running Tests

The test suite covers the matching algorithm, database layer, and API endpoints.

```bash
# Run all tests against a throwaway MongoDB (credentials from configuration.yaml)
DATABASE_HOST=localhost:27117 DATABASE_NAME=launchpad_test cargo test

# Run a specific test module
DATABASE_HOST=localhost:27117 DATABASE_NAME=launchpad_test cargo test --test model
cargo test --test controller   # no MongoDB needed
cargo test --test matching_golden --test body_limit --test configuration   # no MongoDB needed

# Matching benchmark (ignored by default, see Performance)
cargo test --release --test bench_matching -- --ignored --nocapture
```

Always set `DATABASE_NAME` to a test database such as `launchpad_test`: the tests drop the `matches` and `market_data` collections of that database. Some tests use their own databases named `<DATABASE_NAME>_<suffix>` (for example `launchpad_test_stats_qh`) and drop them before running.

**Test coverage:**

| File                  | What is tested                                     |
|-----------------------|----------------------------------------------------|
| `tests/auth_model.rs` | Password hashing rules, service user upsert        |
| `tests/body_limit.rs` | `json_body_limit`: `413` JSON over the limit, a 10 MiB batch accepted with the default |
| `tests/configuration.rs` | `json_body_limit` default, file and environment override, validation |
| `tests/controller.rs` | Pay-As-Bid algorithm, partial fills, several time slots, order and match ids, statistics |
| `tests/matching_golden.rs` | Optimized matching equals the pre-MS-10 implementation on seeded random batches (bit for bit); orders with identical content |
| `tests/model.rs`      | MongoDB insert, filter, paging, aggregation (resolutions, time zones), counts, upsert, indexes |
| `tests/views.rs`      | HTTP endpoints, auth, parameter handling, JSON error bodies |

> Model, view and the `auth_model` upsert tests require a running MongoDB instance configured via `configuration.yaml` (or environment variables). The view tests for the `/ready` 503, the `500` database-error responses and the `400` malformed body / query string responses point at an unreachable address (`127.0.0.1:1`) and do not need MongoDB.
