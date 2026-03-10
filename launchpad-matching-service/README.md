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
  - [Obtain Token](#post-token)
  - [Match Orders](#post-match)
  - [Filter Matches](#get-matches)
  - [Market Statistics](#get-statistics)
  - [Get Markets](#get-markets)
- [Matching Algorithm](#matching-algorithm)
- [Data Models](#data-models)
- [Running Tests](#running-tests)

---

## Overview

The Launchpad Matching Service is responsible for:

1. **Order Matching** — Accepts lists of open bids and offers and matches them using the Pay-As-Bid algorithm.
2. **Trade Storage** — Persists all matches and market statistics in MongoDB.
3. **Market Analytics** — Provides time-series statistics on traded energy, average trade rates, and market success rates with configurable aggregation resolutions (per-slot, daily, monthly).
4. **Market Discovery** — Exposes an endpoint to list unique market IDs associated with a user.

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
│   ├── main.rs                  # Application entry point
│   ├── lib.rs                   # Library root
│   ├── configuration.rs         # Configuration loading
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
│   ├── controller.rs            # Matching algorithm tests
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

- Rust (2024 edition or later) — [rustup.rs](https://rustup.rs/)
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
```

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

### Running with Docker

Two Docker images are provided:

- **`Dockerfile.matching-service`** — Builds and runs the Rust service.
- **`Dockerfile.caddy`** — Builds a Caddy reverse proxy with rate limiting (100 req/s per IP) that forwards traffic to the service on port 9876.

A Docker Compose file is available for cluster deployment:

```bash
docker compose up
```

---

## Authentication

All endpoints except `/health-check` and `/token` require a **Bearer JWT token** in the `Authorization` header.

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

---

## API Reference

### `GET /health-check`

Returns `200 OK` if the service is running. No authentication required.

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

Submit bids and offers to be matched using the Pay-As-Bid algorithm. Resulting matches are persisted in MongoDB and market statistics are updated atomically.

**Authentication:** Required

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
      "user_id": "user-123",
      "market_id": "market-xyz",
      "time_slot": 1700000000,
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

`residual_bid` / `residual_offer` are non-null when an order was only partially filled, containing the remaining quantity.

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

**Response `200`:** Array of match objects (sorted by `time_slot` ascending).

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
| `resolution` | No       | String  | `no_aggregation` (default), `day`, `month`       |

**Resolution values:**
- `no_aggregation` — one data point per time slot
- `day` — aggregate into 24-hour buckets (86 400 s)
- `month` — aggregate into 30-day buckets (2 592 000 s)

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
  "success_rate": 0.833
}
```

`success_rate` = `matched_energy / (matched_energy + unmatched_energy)`

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
1. `offer_energy_rate <= bid_energy_rate` — the offer is priced at or below the bid.
2. `bid.area_uuid != offer.area_uuid` — parties must be from different grid areas.
3. Both orders still have remaining energy > ε (0.000001 kWh).

### Execution

- The algorithm iterates through sorted offers (outer loop) and, for each offer, iterates through sorted bids (inner loop).
- `selected_energy = min(remaining_bid_energy, remaining_offer_energy)`
- The **matched energy rate equals the bid's energy rate** (pay-as-bid principle).
- Available energy for each order is decremented after each partial or full match.

### Residual Orders

When an order is only partially matched, a **residual order** is created with:
- Energy = original energy − matched energy
- Nonce incremented by 1

Residuals are stored alongside the match record.

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
  user_id: String,
  market_id: String,
  time_slot: u64,         // Unix timestamp
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

---

## Running Tests

The test suite covers the matching algorithm, database layer, and API endpoints.

```bash
# Run all tests
cargo test

# Run a specific test module
cargo test --test controller
cargo test --test model
cargo test --test views
```

**Test coverage:**

| File                  | What is tested                                     |
|-----------------------|----------------------------------------------------|
| `tests/controller.rs` | Pay-As-Bid algorithm, partial fills, statistics    |
| `tests/model.rs`      | MongoDB insert, filter, aggregation, upsert        |
| `tests/views.rs`      | HTTP endpoints, auth, parameter handling           |

> Model and view tests require a running MongoDB instance configured via `configuration.yaml` (or environment variables).
