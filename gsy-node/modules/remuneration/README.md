# Remuneration Module

## Overview

The Remuneration module manages the financial interactions within a decentralized energy exchange system. It facilitates the tracking of energy communities, prosumers, and financial transactions, while ensuring operations are governed by a designated custodian.

This module is integral to maintaining accountability, enabling transparent record-keeping, and simplifying energy trade settlements among participants, including specialized flexibility service payments.

## Features

### Administrative Functions

- **Custodian Management**: A designated custodian has administrative privileges to manage all aspects of the system
- **Community Management**: Registration and management of energy communities and their associated DSOs (Distribution System Operators)
- **Prosumer Association**: Management of prosumers and their community affiliations

### Financial Operations

- **Balance Tracking**: Maintains balances of participants in the system
- **Payment Processing**: Handles payments between different entities:
  - Intra-community payments (between prosumers in the same community)
  - Inter-community payments (between different communities)
- **Flexibility Service Settlement**: Calculates payments for flexibility services with incentives/penalties based on performance
- **Adaptive Incentive & Policy System**: Dynamically adjusts:
  - Under-delivery penalty factor (alpha)
  - Over-delivery bonus factor (beta)
  - Under-delivery tolerance (UnderTolerance) via feedback on recent performance

### Flexibility Payment Calculation

The module includes a specialized settlement mechanism for flexibility services with:

- **Base Payment**: min(requested, delivered) * price
- **Under-delivery Penalties**: Applied when delivered flexibility is less than requested beyond the under-delivery tolerance
- **Over-delivery Bonuses**: Applied when delivered flexibility exceeds requested amount beyond the over-delivery tolerance

Parameters (all fixed-point with 1.0 = 1_000_000):
- **Alpha**: Under-delivery penalty scaler
- **Beta**: Over-delivery bonus scaler
- **UnderTolerance**: Allowed fractional shortfall before penalty (per request)
- **OverTolerance**: Allowed fractional excess before bonus

### Adaptive Parameter Mechanism

Adaptive control now covers alpha, beta, and under-delivery tolerance via two extrinsics:

1. `set_adaptation_params(u_ref, o_ref, k_alpha, k_beta, k_under_tol, window_size)`
2. `dynamically_adapt_parameters(u_measurements, o_measurements)`

Definitions:
- `u_ref`: Target under-delivery deviation (reference benchmark)
- `o_ref`: Target over-delivery deviation
- `k_alpha`: Gain for alpha adaptation
- `k_beta`: Gain for beta adaptation
- `k_under_tol`: Gain for adaptive under-tolerance scaling
- `window_size` (`N`): Exact number of measurement samples required per adaptation
- All measurements & parameters use the same fixed-point scaling (1e6 = 1.0)

Given the last N measurements:
```
u_avg = mean(u_measurements)
o_avg = mean(o_measurements)

alpha_{t+1} = clamp( alpha_t * ( 1 + k_alpha * (u_avg - u_ref) ) )
beta_{t+1}  = clamp( beta_t  * ( 1 + k_beta  * (o_avg - o_ref) ) )
underTol_{t+1} = clamp( underTol_t * ( 1 - k_under_tol * (u_avg - u_ref) ) )
```
Where `clamp` applies `[0, u64::MAX]` after fixed-point arithmetic; negative intermediate factors drive values toward zero.

Internal integer form (F = 1_000_000):
```
factor_a  = F + (k_alpha     * (u_avg - u_ref)) / F
factor_b  = F + (k_beta      * (o_avg - o_ref)) / F
factor_ut = F - (k_under_tol * (u_avg - u_ref)) / F
new_alpha = alpha * factor_a  / F
new_beta  = beta  * factor_b  / F
new_under = underTol * factor_ut / F
```
> NOTE: Only UnderTolerance is adapted; OverTolerance is currently static (manual updates via `update_over_tolerance`).

### Example Adaptation Workflow

```rust
// Custodian sets adaptation policy (window size = 3 samples)
Remuneration::set_adaptation_params(
    origin,
    400_000, // u_ref (0.4)
    300_000, // o_ref (0.3)
    100_000, // k_alpha (0.1)
    200_000, // k_beta  (0.2)
    050_000, // k_under_tol (0.05)
    3        // window size
);

// Later, adapt using last 3 deviation samples
Remuneration::dynamically_adapt_parameters(
    origin,
    vec![500_000, 600_000, 700_000], // under-delivery samples
    vec![400_000, 500_000, 600_000], // over-delivery samples
);

let alpha_now = Remuneration::alpha();
let beta_now  = Remuneration::beta();
let under_tol = Remuneration::under_tolerance();
```

### Extrinsics Summary (Call Indices)
| Index | Extrinsic | Purpose |
|-------|-----------|---------|
| 1 | update_custodian | Set / change custodian |
| 2 | add_community | Register community |
| 3 | remove_community | Remove community |
| 4 | add_prosumer | Register prosumer to community |
| 5 | remove_prosumer | Deregister prosumer |
| 6 | update_prosumer | Move prosumer to another community |
| 7 | add_payment | Register payment (intra or inter) |
| 8 | set_balance | Custodian sets internal balance |
| 13 | update_alpha | Manual alpha update |
| 14 | update_beta | Manual beta update |
| 15 | update_under_tolerance | Manual UnderTolerance update |
| 16 | update_over_tolerance | Manual OverTolerance update |
| 17 | settle_flexibility_payment | Compute & transfer flexibility payment |
| 18 | set_adaptation_params | Configure adaptation policy |
| 19 | dynamically_adapt_parameters | Adapt alpha, beta, under tolerance |

### Usage Examples

#### Setup
```rust
Remuneration::update_custodian(origin, admin);

// Settlement parameters
Remuneration::update_alpha(origin, 500_000);        // 0.5
Remuneration::update_beta(origin, 200_000);         // 0.2
Remuneration::update_under_tolerance(origin, 100_000); // 0.1
Remuneration::update_over_tolerance(origin, 150_000);  // 0.15

// Adaptive policy (optional)
Remuneration::set_adaptation_params(
    origin,
    500_000, // u_ref
    300_000, // o_ref
    100_000, // k_alpha
    200_000, // k_beta
    050_000, // k_under_tol
    5        // window size
);

// Add entities
Remuneration::add_community(origin, community, dso, owner);
Remuneration::add_prosumer(origin, prosumer, community);
```

#### Payments & Settlement
```rust
// Intra-community payment
Remuneration::add_payment(origin, receiver, 1_000u128.into(), INTRA_COMMUNITY);

// Inter-community payment
Remuneration::add_payment(origin, other_community, 5_000u128.into(), INTER_COMMUNITY);

// Flexibility payment
Remuneration::settle_flexibility_payment(
    origin,
    provider,
    100,   // requested
    92,    // delivered
    5,     // price
    INTRA_COMMUNITY
);

// Adaptive update after collecting N samples
Remuneration::dynamically_adapt_parameters(origin, u_samples, o_samples);
```

## Flexibility Settlement Calculation

```
base_payment          = min(requested, delivered) * price
threshold_under       = UnderTolerance * requested / 1_000_000
threshold_over        = OverTolerance  * requested / 1_000_000
under_excess          = max(0, (requested - delivered) - threshold_under)
under_delivery_penalty= alpha * under_excess * price / 1_000_000
over_excess           = max(0, (delivered - requested) - threshold_over)
over_delivery_bonus   = beta  * over_excess  * price / 1_000_000
final_amount          = base_payment - under_delivery_penalty + over_delivery_bonus
```

## Adaptive Parameter Validation

- `window_size > 0`
- Measurement vectors:
  - Non-empty
  - Same length
  - Length == `window_size`
- Negative scaling => clamp to 0
- Multiplication overflow => clamp to `u64::MAX`
- UnderTolerance adaptation only (OverTolerance is manual)

## Events
- `CustodianUpdated`
- `CommunityAdded` / `CommunityRemoved`
- `ProsumerAdded` / `ProsumerRemoved`
- `PaymentAdded`
- `BalanceSet`
- `AlphaUpdated` / `BetaUpdated`
- `UnderToleranceUpdated` / `OverToleranceUpdated`
- `FlexibilitySettled`
- `AdaptationParamsUpdated` (now includes `k_under_tol`)
- `AlphaBetaAdapted` (emitted after dynamic adaptation — may be accompanied by `UnderToleranceUpdated` if it changes)

## Errors
Authorization:
- `NotCustodian`, `NotAllowedToManageProsumers`

Entity / Relationship:
- `SenderNotProsumer`, `ReceiverNotProsumer`, `NotACommunity`, `DifferentCommunities`

Validation:
- `SameSenderReceiver`, `InsufficientBalance`, `PaymentTypeNotAllowed`
- `InvalidWindowSize`, `EmptyMeasurements`, `MismatchedMeasurements`, `MeasurementsExceedWindow`

## Testing

Coverage includes everything previously documented plus:
- Dual tolerance behavior (separate under & over) in settlement
- Dynamic under tolerance adaptation scenarios:
  - Decrease when deviation above reference
  - Increase when deviation below reference
  - Clamp to zero edge case
- Rename path: `dynamically_adapt_parameters` replacing legacy `adapt_alpha_beta`

Run tests:
```bash
cargo test -p remuneration
```

## Integration
Integrates with:
- `orderbook_registry` (participant registry)
- Other settlement / market pallets in the runtime
- Standard FRAME pallets (`frame_system`, `pallet_balances`)

## Notes & Future Extensions
- OverTolerance could be made adaptive analogously (gain + adaptation rule)
- Additional safety guards (e.g., min/max bounds on adaptive parameters) can be introduced if governance requires tighter control
- Event filtering dashboards should listen for `UnderToleranceUpdated` following adaptation cycles
