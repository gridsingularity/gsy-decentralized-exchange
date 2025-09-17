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

### Flexibility Payment Calculation (Linear + Tolerances)

The module includes a settlement mechanism for flexibility services with:

- **Base Payment**: min(requested, delivered) * price
- **Under-delivery Penalties**: Applied when delivered flexibility is less than requested beyond the under-delivery tolerance
- **Over-delivery Bonuses**: Applied when delivered flexibility exceeds requested amount beyond the over-delivery tolerance

Parameters (all fixed-point with 1.0 = 1_000_000):
- **Alpha**: Under-delivery penalty scaler
- **Beta**: Over-delivery bonus scaler
- **UnderTolerance**: Allowed fractional shortfall before penalty (per request)
- **OverTolerance**: Allowed fractional excess before bonus

### Piecewise Quadratic Under-Delivery Penalty (PW Quad)

Besides the linear/tolerance model, the module supports a piecewise quadratic penalty for under-delivery. In this variant, over-delivery does not grant any bonus (it is ignored). The final payment is:

```
base_payment = min(E_r, E_m) * price
penalty_value = P(E_r, E_m) * price
final_amount = max(0, base_payment - penalty_value)
```

Where the penalty in energy units, P(E_r, E_m), is computed via a piecewise rule using two thresholds derived from the requested energy and configured epsilons:

- Fixed-point scale F = 1_000_000
- e1 = E_r * (1 - eps1/F)
- e2 = E_r * (1 - eps2/F)

Piecewise penalty (energy units):
```
if E_m >= e1:
    P = 0
elif e2 <= E_m < e1:
    P = alpha_piecewise * (e1 - E_m)
else:  # E_m < e2
    P = alpha_piecewise * (e1 - E_m) + alpha_piecewise * (e2 - E_m)^2
```
Notes:
- eps1 and eps2 are fixed-point fractions in [0, 1] with F = 1_000_000
- alpha_piecewise is a dimensionless integer scaling factor applied directly in energy units
- Over-delivery is ignored (no bonus added)

Related storage parameters and extrinsics:
- alpha_piecewise: `update_alpha_piecewise(new_value: u64)`
- eps_piecewise_1: `update_eps_piecewise_1(new_value: u64)` (fixed-point)
- eps_piecewise_2: `update_eps_piecewise_2(new_value: u64)` (fixed-point)

Settlement extrinsic using the PW Quad penalty:
- `settle_flexibility_payment_with_pw_quad_penalty(receiver, requested, delivered, price, payment_type)`

Helper (read-only) API:
- `calc_piecewise_quadratic_penalty(requested: u64, delivered: u64) -> u64`
  - Returns the penalty in energy units P(E_r, E_m) computed via the above piecewise rule

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
| 17 | settle_flexibility_payment | Linear model: compute & transfer flexibility payment |
| 18 | set_adaptation_params | Configure adaptation policy |
| 19 | dynamically_adapt_parameters | Adapt alpha, beta, under tolerance |
| 20 | update_alpha_piecewise | Set alpha_piecewise for PW Quad penalty |
| 21 | update_eps_piecewise_1 | Set eps1 (fixed-point) for PW Quad |
| 22 | update_eps_piecewise_2 | Set eps2 (fixed-point) for PW Quad |
| 23 | settle_flexibility_payment_with_pw_quad_penalty | PW Quad model: compute & transfer |

### Usage Examples

#### Setup
```rust
Remuneration::update_custodian(origin, admin);

// Linear settlement parameters
Remuneration::update_alpha(origin, 500_000);           // 0.5
Remuneration::update_beta(origin, 200_000);            // 0.2
Remuneration::update_under_tolerance(origin, 100_000); // 0.1
Remuneration::update_over_tolerance(origin, 150_000);  // 0.15

// Piecewise quadratic parameters
Remuneration::update_alpha_piecewise(origin, 1);       // integer coefficient
Remuneration::update_eps_piecewise_1(origin, 200_000); // 0.2
Remuneration::update_eps_piecewise_2(origin, 400_000); // 0.4

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

// Flexibility payment - Linear model (with over/under tolerances)
Remuneration::settle_flexibility_payment(
    origin,
    provider,
    100,   // requested
    92,    // delivered
    5,     // price
    INTRA_COMMUNITY
);

// Flexibility payment - Piecewise quadratic under-delivery (no over-delivery bonus)
Remuneration::settle_flexibility_payment_with_pw_quad_penalty(
    origin,
    provider,
    100,   // requested
    70,    // delivered
    10,    // price
    INTRA_COMMUNITY
);
```

## Flexibility Settlement Calculation (Linear Model)

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
- Piecewise quadratic under-delivery penalty:
  - All three branches and their boundaries (E_m ≥ e1, e2 ≤ E_m < e1, E_m < e2)
  - Over-delivery ignored (no bonus)
  - Saturation behavior when penalty exceeds base

Run tests:
```bash
cargo test -p remuneration
```

### Test Suite Overview

- Administrative and registry
  - custodian_management
  - community_management
  - prosumer_management

- Payments and balances
  - intra_community_payment_ok
  - inter_community_payment_ok
  - payment_err_insufficient_balance
  - payment_err_intra_prosumers_belonging_to_different_communities
  - payment_err_inter_actors_not_being_communities

- Settlement parameters and tolerances (linear model)
  - update_settlement_parameters
  - settle_flexibility_basic
  - settle_flexibility_under_delivery
  - settle_flexibility_over_delivery
  - settle_flexibility_with_tolerance
  - settle_flexibility_complex_scenario
  - settle_flexibility_errors
  - settle_flexibility_inter_community
  - settle_flexibility_dual_tolerances

- Piecewise quadratic penalty (PW Quad)
  - piecewise_parameters_management
  - calc_piecewise_quadratic_penalty_branches_and_boundaries
  - settle_flexibility_payment_with_pw_quad_penalty

- Adaptive mechanism (alpha/beta/under tolerance)
  - adaptation_set_params_success_and_event
  - adaptation_set_params_not_custodian_fails
  - adaptation_set_params_zero_window_fails
  - adaptation_alpha_beta_success_updates_and_events
  - adaptation_alpha_beta_not_custodian_fails
  - adaptation_alpha_beta_invalid_window_size_when_not_set
  - adaptation_alpha_beta_empty_measurements_fails
  - adaptation_alpha_beta_mismatched_lengths_fail
  - adaptation_alpha_beta_window_size_mismatch_fail
  - adaptation_alpha_beta_negative_factor_clamps_to_zero
  - adaptation_alpha_beta_overflow_clamps_to_u64_max

- Runtime integrity (from mock runtime)
  - mock::__construct_runtime_integrity_test::runtime_integrity_tests
  - mock::test_genesis_config_builds

### How to list tests

```bash
# List all tests in this crate
cargo test -p remuneration -- --list --format=pretty
```

### Sample results

Example output from a local run (will vary slightly by environment):

```text
running 31 tests
...............................
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

Running unittests src/lib.rs (.../target/debug/deps/remuneration-*)
mock::__construct_runtime_integrity_test::runtime_integrity_tests: test
mock::test_genesis_config_builds: test
... (remaining test names) ...

31 tests, 0 benchmarks
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
