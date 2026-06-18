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
  - Signed over-delivery adjustment factor (beta)
  - Under-delivery tolerance (UnderTolerance) via feedback on recent performance

### Flexibility Payment Calculation (Linear + Tolerances)

The module includes a settlement mechanism for flexibility services with:

- **Base Payment**: min(requested, delivered) * price
- **Under-delivery Penalties**: Applied when delivered flexibility is less than requested beyond the under-delivery tolerance
- **Over-delivery Adjustments**: Applied when delivered flexibility exceeds requested amount beyond the over-delivery tolerance. Positive beta rewards over-delivery, zero beta applies no adjustment, and negative beta penalizes over-delivery.

Parameters (all fixed-point with 1.0 = 1_000_000):
- **Alpha**: Under-delivery penalty scaler
- **Beta**: Signed over-delivery adjustment scaler (`+1_000_000 = +1.0`, `0 = no adjustment`, `-1_000_000 = -1.0`)
- **UnderTolerance**: Allowed fractional shortfall before penalty (per request)
- **OverTolerance**: Allowed fractional excess before any over-delivery adjustment

### Piecewise Quadratic Under-Delivery Penalty (PW Quad)

Besides the linear/tolerance model, the module supports a piecewise quadratic penalty for under-delivery. In this variant, over-delivery does not grant any bonus and is ignored by the penalty helper.

The implementation uses fixed-point scale `F = 1_000_000` for the epsilon thresholds:

```
e1 = (F - eps1) * E_r / F
e2 = (F - eps2) * E_r / F
```

The divisions use integer truncation. The empirical penalty score `P(E_r, E_m)` is:

```
if E_m >= e1:
    P = 0
elif e2 <= E_m < e1:
    P = AlphaPiecewise * (e1 - E_m)
else:  # E_m < e2
    P = AlphaPiecewise * (e1 - E_m) + AlphaPiecewise * (e2 - E_m)^2
```

Settlement then applies:

```
BasePayment = min(E_r, E_m) * price
PenaltyValue = P(E_r, E_m) * price
Settlement = max(0, BasePayment - PenaltyValue)
```

Parameter conventions:

| Parameter | Convention |
| --- | --- |
| AlphaPiecewise | Raw integer empirical scaling parameter |
| EpsPiecewise1 | Fixed-point fraction, 1_000_000 = 1.0 |
| EpsPiecewise2 | Fixed-point fraction, 1_000_000 = 1.0 |

`AlphaPiecewise = 1` means multiplier `1`. `AlphaPiecewise = 1_000_000` does not mean `1.0`.

The piecewise-quadratic approach is retained as an empirical two-threshold penalty model. AlphaPiecewise is a raw integer calibration parameter rather than a fixed-point coefficient, and its numerical value is specific to the selected energy unit. The formulation is value-continuous at both thresholds and provides a linear penalty for moderate under-delivery followed by a stronger quadratic escalation for severe under-delivery. Because the quadratic term is not normalised, the model is not invariant to changes in the energy unit; pilot deployments must therefore use a fixed energy representation and calibrate the coefficient accordingly. The model is retained alongside the normalised hybrid approach because the two methods represent distinct experimental remuneration strategies.

The curve is an empirical scoring rule, not a physical law. The linear term scales with the energy deviation, while the quadratic term scales with the square of the severe deviation. A single coefficient is used for both terms, so the model is not dimensionally normalised. The energy unit must be fixed by the pilot configuration and used consistently during calibration and operation. Coefficients calibrated for one energy unit cannot be reused unchanged with another; for example, changing from kWh to Wh changes the quadratic term disproportionately, so `AlphaPiecewise` must be recalibrated when the energy representation changes.

The penalty is zero at and above `e1`, value-continuous at `e1`, and value- and slope-continuous at `e2`. There is a kink at `e1`, then stronger quadratic escalation below `e2`.

The implementation uses saturating arithmetic. Extreme parameters or deviations may saturate the penalty score, and the final payment then floors to zero. Overflow does not create an inflated positive payment. Unlike the hybrid path, this model does not return an arithmetic-overflow error.

Compared with the hybrid model, PW Quad is empirical, uses two under-delivery thresholds, applies a raw-integer alpha, and depends on energy-unit-specific calibration. The hybrid model is normalised, uses fixed-point coefficients, checked arithmetic, and signed over-delivery adjustment, making it more portable across energy scales. Retaining both models is intentional because they represent distinct modelling approaches.

Example with `E_r = 100`, `E_m = 75`, `eps1 = 0.10`, `eps2 = 0.20`, and `AlphaPiecewise = 1`:

```
e1 = 90
e2 = 80
linear = 90 - 75 = 15
quadratic = (80 - 75)^2 = 25
P = 15 + 25 = 40
```

With `price = 50`, `PenaltyValue = 2_000`, `BasePayment = min(100, 75) * 50 = 3_750`, and `Settlement = 1_750`.

Related storage parameters and extrinsics:
- Storage: `alpha_piecewise`, `eps_piecewise_1`, `eps_piecewise_2`
- Extrinsic: `set_piecewise_parameters(new_alpha_pw: u64, new_eps1: u64, new_eps2: u64)` (epsilon values are fixed-point)
- Settlement extrinsic: `settle_flexibility_payment_with_pw_quad_penalty(receiver, requested, delivered, price, payment_type)`
- Helper API: `calc_piecewise_quadratic_penalty(requested: u64, delivered: u64) -> u64`

### Hybrid Settlement Model (Corrected D6.4)

The hybrid model combines a *tolerance band* with an asymmetric over/under-delivery
net adjustment applied on top of the base payment.

> **Interpretation note.** D6.4 Equation 3 as printed is mathematically inconsistent
> (it raises a negative base to a fractional power, flips signs, is discontinuous at the
> tolerance edges and is dimensionally ambiguous). The implementation below is a
> **corrected interpretation preserving the intended qualitative shape** of D6.4 — it is
> **not** a mathematically equivalent rewriting of the printed equation, and it does
> **not** implement fractional exponents.

#### Fixed-point convention

All coefficients use the module-wide fixed-point scale `F = FIXED_POINT_SCALE = 1_000_000`
(so `1_000_000` represents `1.0`). Energies (`E_r`, `E_m`), price `p` and balances are
plain integers. All intermediate arithmetic uses checked `u128`/`i128` operations; there
are no narrowing casts, no wrapping, no floating point and no panics.

#### Boundaries (closed tolerance band)

```
lower = E_r * (F - eps) / F
upper = E_r * (F + eps) / F
```

The band `lower <= E_m <= upper` is **closed**: at either exact boundary the net
adjustment is exactly zero.

#### Net energy adjustment A

- Over-delivery, `E_m > upper`:

```
A_over = gamma_over * (E_m - upper) / F          (signed energy)
```

  `gamma_over` is signed fixed point: positive rewards over-delivery, zero applies no
  adjustment, negative penalises over-delivery.

- Under-delivery, `E_m < lower` and `E_r > 0` (quadratic, `n == 2` only):

```
A_under = -gamma_under * (lower - E_m)^2 / (F * E_r)   (signed energy, always <= 0)
```

  `gamma_under` is an unsigned fixed-point magnitude; the formula supplies the negative
  sign, so under-delivery can never become a reward. The penalty grows quadratically with
  the shortfall.

- Flat band, `lower <= E_m <= upper`, and the degenerate case `E_r == 0`:

```
A = 0
```

#### Base-payment integration and final clamp

The existing base-payment convention is preserved:

```
BasePayment = min(E_r, E_m) * p
Settlement  = max(0, BasePayment + A * p)
```

`A` is a *signed energy* adjustment; multiplying by price gives a *signed monetary*
adjustment. Only the **final settlement** is clamped to zero — the base-payment semantics
are unchanged.

#### Rounding

All divisions use integer truncation toward zero, consistent with the existing standard
and piecewise settlement helpers. Sub-unit penalties/bonuses therefore round down in
magnitude.

#### Parameters, validation and API

Storage (re-typed from the previous unused scaffolding):

| Storage | Type | Meaning |
|---------|------|---------|
| `GammaOverHybrid` | `i64` | signed fixed-point over-delivery coefficient |
| `GammaUnderHybrid` | `u64` | unsigned fixed-point under-delivery penalty magnitude |
| `EpsHybrid` | `u64` | fixed-point tolerance fraction in `[0, F]` |
| `NHybrid` | `u32` | integer exponent (only `n == 2` is implemented) |

Setter `set_hybrid_model_parameters(gamma_over: i64, gamma_under: u64, eps: u64, n: u32)`
(call index **21**, custodian-only) validates **all** fields before writing any storage:

- `eps`: accepted for `0 <= eps <= F` (`eps = 0` collapses the band to `lower = upper = E_r`);
  `eps > F` is rejected with `InvalidHybridEpsilonRange`.
- `n`: only `n == 2` is accepted; `0`, `1`, `3`, … are rejected with `InvalidHybridExponent`.
- `gamma_over` may be any `i64`, `gamma_under` may be any `u64`; there are no arbitrary
  economic caps — overflow during settlement returns an explicit error instead of saturating.

An invalid setter call leaves all four parameters unchanged and emits no event.
`query_hybrid_model_params() -> (i64, u64, u64, u32)` reads them back.

Settlement extrinsic
`settle_flexibility_payment_with_hybrid_adjustment(receiver, requested, delivered, price, payment_type)`
(call index **24**, `#[transactional]`): reads the four hybrid parameters from storage
(they are never supplied by the caller), computes the settlement via the pure helper, and
applies it through the existing checked balance conversion and `add_payment` ledger path.
It emits `HybridFlexibilitySettled { requester, provider, requested, delivered, price,
calculated_amount, net_adjustment }`, where **`net_adjustment` is the signed *monetary*
adjustment** (currency units, i.e. the energy adjustment already multiplied by price)
applied before the final clamp to zero.

Pure helper (crate-internal):
`calculate_hybrid_settlement_amount(requested, delivered, price, gamma_over, gamma_under, eps, n) -> Result<(u128, i128), Error<T>>`
returns `(final_settlement, net_monetary_adjustment)` and writes no storage.

#### Metadata / migration note

Re-typing `GammaOverHybrid` to `i64` and `NHybrid` to `u32` changes storage and metadata
layout, but **no migration is required**: the previous hybrid scaffolding was never
exercised by any running chain, so there is no encoded state to migrate.

#### Example

With `F = 1_000_000`, `E_r = 100`, `eps = 100_000` (10%) → `lower = 90`, `upper = 110`.

- Over-delivery reward: `E_m = 120`, `p = 5`, `gamma_over = +1_000_000` (=`+1.0`):
  `A_over = +10` energy → `+50` monetary; `BasePayment = 100*5 = 500` → `Settlement = 550`.
- Over-delivery penalty: same inputs with `gamma_over = -1_000_000` → `Settlement = 450`.
- Quadratic under-delivery: `E_m = 70`, `p = 5`, `gamma_under = 1_000_000`:
  `(lower - E_m) = 20`, `A_under = -1_000_000 * 400 / (1_000_000 * 100) = -4` energy →
  `-20` monetary; `BasePayment = 70*5 = 350` → `Settlement = 330`. Halving the shortfall to
  `10` reduces the penalty four-fold to `-1` energy (quadratic shape).
- Clamp: `E_r = 100`, `E_m = 10`, `p = 1`, `eps = 0`, `gamma_under = 1_000_000`:
  penalty `-81` exceeds `BasePayment = 10`, so `Settlement = 0`.

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
beta_{t+1}  = checked_signed( beta_t * ( 1 + k_beta * (o_avg - o_ref) ) )
underTol_{t+1} = clamp( underTol_t * ( 1 - k_under_tol * (u_avg - u_ref) ) )
```
Alpha remains unsigned and clamps to `[0, u64::MAX]` after fixed-point arithmetic. Beta is signed (`i64`) and is adapted with the same multiplicative factor; it may remain positive or negative, become zero, or cross sign when the factor is negative. Beta adaptation fails atomically if the signed result cannot be represented.

Internal integer form (F = 1_000_000):
```
factor_a  = F + (k_alpha     * (u_avg - u_ref)) / F
factor_b  = F + (k_beta      * (o_avg - o_ref)) / F
factor_ut = F - (k_under_tol * (u_avg - u_ref)) / F
new_alpha = alpha * factor_a  / F
new_beta  = beta  * factor_b  / F
new_under = underTol * factor_ut / F
```
> NOTE: Only UnderTolerance is adapted; OverTolerance is currently set manually via `set_main_parameters`.

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
let beta_now  = Remuneration::beta(); // signed fixed-point value
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
| 13 | set_main_parameters | Set alpha, beta, under & over tolerances atomically |
| 17 | settle_flexibility_payment | Linear model: compute & transfer flexibility payment |
| 18 | set_adaptation_params | Configure adaptation policy |
| 19 | dynamically_adapt_parameters | Adapt alpha, beta, under tolerance |
| 20 | set_piecewise_parameters | Set alpha_pw, eps1, eps2 for PW Quad |
| 21 | set_hybrid_model_parameters | Set gamma_over (i64), gamma_under, eps, n for the hybrid model |
| 23 | settle_flexibility_payment_with_pw_quad_penalty | PW Quad model: compute & transfer |
| 24 | settle_flexibility_payment_with_hybrid_adjustment | Hybrid model: compute & transfer |

### Usage Examples

#### Setup
```rust
Remuneration::update_custodian(origin, admin);

// Linear settlement parameters (all at once)
Remuneration::set_main_parameters(origin, 500_000, -200_000, 100_000, 150_000);

// Piecewise quadratic parameters
Remuneration::set_piecewise_parameters(origin, 1, 200_000, 400_000);

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
over_delivery_adjustment = beta * over_excess * price / 1_000_000
final_amount             = max(0, base_payment - under_delivery_penalty + over_delivery_adjustment)
```

Beta is signed:
- `beta > 0`: the over-delivery adjustment is a bonus.
- `beta = 0`: no over-delivery adjustment is applied.
- `beta < 0`: the over-delivery adjustment is a penalty, and the final payment is bounded below by zero.

Compatibility note: signed beta changes `Beta` storage from `u64` to `i64` and changes the `set_main_parameters` beta argument and beta-related events from unsigned to signed. Existing chain state encoded with the old `u64` beta storage would require a storage migration before upgrade; no migration is included in this research branch.

## Adaptive Parameter Validation

- `window_size > 0`
- Measurement vectors:
  - Non-empty
  - Same length
  - Length == `window_size`
- Negative alpha scaling => clamp alpha to 0
- Alpha multiplication overflow => clamp alpha to `u64::MAX`
- Signed beta adaptation may cross zero; signed beta overflow or representability failure rejects the adaptive update atomically
- UnderTolerance adaptation only (OverTolerance is manual via `set_main_parameters`)

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
  - adaptation_alpha_overflow_clamps_to_u64_max_and_signed_beta_updates
  - adaptation_signed_beta_decreases_while_remaining_positive
  - adaptation_signed_beta_positive_crosses_to_negative
  - adaptation_signed_beta_negative_remains_negative_under_positive_factor
  - adaptation_signed_beta_negative_crosses_to_positive
  - adaptation_signed_beta_zero_remains_zero
  - adaptation_signed_beta_overflow_fails_atomically

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
