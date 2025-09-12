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
- **Adaptive Incentive Policy (NEW)**: Dynamically adjusts the penalty (alpha) and bonus (beta) factors based on recent under-/over-delivery performance history

### Flexibility Payment Calculation

The module includes a specialized settlement mechanism for flexibility services with:

- **Base Payment**: Calculated as minimum of requested and delivered flexibility multiplied by price
- **Under-delivery Penalties**: Applied when delivered flexibility is less than requested (beyond tolerance)
- **Over-delivery Bonuses**: Applied when delivered flexibility exceeds requested amount

Parameters for flexibility settlement:
- **Alpha**: Controls the penalty factor for under-delivery (fixed-point value)
- **Beta**: Controls the bonus factor for over-delivery (fixed-point value)
- **Tolerance**: Defines the acceptable deviation threshold without penalties (fixed-point value)

### Adaptive Alpha / Beta Mechanism

The module supports a closed-loop update of the incentive parameters (alpha, beta) via two new extrinsics:

1. `set_adaptation_params(u_ref, o_ref, k_alpha, k_beta, window_size)`
2. `adapt_alpha_beta(u_measurements, o_measurements)`

All adaptive parameters use the same fixed-point convention (1.0 = 1_000_000).

Definitions:
- `u_ref`: Reference (target) average under-delivery deviation
- `o_ref`: Reference (target) average over-delivery deviation
- `k_alpha`: Gain factor controlling sensitivity of alpha updates
- `k_beta`: Gain factor controlling sensitivity of beta updates
- `window_size` (`N`): Required number of measurements for each adaptation call

On each adaptation call with N measurements:
- Compute averages: `u_avg = mean(u_measurements)`, `o_avg = mean(o_measurements)`
- Update rules (fixed-point arithmetic):

```
alpha_{t+1} = clamp_0_max( alpha_t * (1 + k_alpha * (u_avg - u_ref)) )
beta_{t+1}  = clamp_0_max( beta_t  * (1 + k_beta  * (o_avg - o_ref)) )
```

Where the internal representation uses integer math with factor F = 1_000_000:
```
factor_a = F + (k_alpha * (u_avg - u_ref)) / F
new_alpha = (alpha * factor_a) / F
```
(analogous for beta). Negative intermediate results are clamped to 0; values that would overflow `u64` are clamped to `u64::MAX`.

### Example Adaptation Workflow

```rust
// Custodian sets adaptation policy (window size = 3 samples)
Remuneration::set_adaptation_params(origin, 400_000, 300_000, 100_000, 200_000, 3);

// Later, provide last 3 measurement samples (fixed-point deviations)
Remuneration::adapt_alpha_beta(
    origin,
    vec![500_000, 600_000, 700_000],  // under-delivery deviations
    vec![400_000, 500_000, 600_000],  // over-delivery deviations
);

// Alpha / Beta now adapted and stored
let alpha_now = Remuneration::alpha();
let beta_now  = Remuneration::beta();
```

### Usage Examples

#### Setting Up the System

```rust
// Set the custodian (privileged administrator)
Remuneration::update_custodian(origin, admin_account);

// Set parameters for flexibility settlement
Remuneration::update_alpha(origin, 500_000);     // 0.5 in fixed-point notation
Remuneration::update_beta(origin, 200_000);      // 0.2 in fixed-point notation
Remuneration::update_tolerance(origin, 100_000); // 0.1 in fixed-point notation

// Configure adaptation policy (optional)
Remuneration::set_adaptation_params(origin, 500_000, 300_000, 100_000, 200_000, 5);

// Add a community
Remuneration::add_community(origin, community_account, dso_account, owner_account);

// Add prosumers to the community
Remuneration::add_prosumer(origin, prosumer_account, community_account);
```

#### Processing Payments

```rust
// Standard payment between prosumers in same community
Remuneration::add_payment(origin, receiver_account, amount, INTRA_COMMUNITY);

// Payment between communities
Remuneration::add_payment(origin, receiver_community, amount, INTER_COMMUNITY);

// Flexibility service payment with performance calculation
Remuneration::settle_flexibility_payment(
    origin,
    provider_account,
    flexibility_requested,  // e.g., 100 units
    flexibility_delivered,  // e.g., 95 units
    price_per_unit,        // e.g., 5 currency units
    INTRA_COMMUNITY
);

// Perform adaptive update of alpha & beta after collecting N deviation samples
Remuneration::adapt_alpha_beta(
    origin,
    under_delivery_samples, // length == configured window_size
    over_delivery_samples   // same length
);
```

## Flexibility Settlement Calculation

The settlement amount is calculated using the following formula:

```
base_payment = min(requested, delivered) * price
under_delivery_penalty = alpha * max(0, requested - delivered - threshold) * price
over_delivery_bonus    = beta  * max(0, delivered - requested - threshold) * price

final_amount = base_payment - under_delivery_penalty + over_delivery_bonus
```

Where:
- `threshold = tolerance * requested / 1_000_000`
- All parameters (alpha, beta, tolerance) use fixed-point arithmetic with 1.0 = 1,000,000

## Adaptive Parameter Constraints & Validation

- `window_size` must be > 0 when set
- Both `u_measurements` & `o_measurements` must:
  - Be non-empty
  - Have identical length
  - Length must equal the configured `window_size`
- Negative adaptation factors clamp result to 0
- Overflowing multiplication clamps result to `u64::MAX`

## Events

The module emits various events for tracking operations:

- `CustodianUpdated`: When the custodian is changed
- `CommunityAdded` / `CommunityRemoved`: When communities are added or removed
- `ProsumerAdded` / `ProsumerRemoved`: When prosumers are added or removed
- `PaymentAdded`: When a standard payment is processed
- `BalanceSet`: When an account's balance is manually updated by the custodian
- `AlphaUpdated` / `BetaUpdated` / `ToleranceUpdated`: When settlement parameters are changed manually
- `FlexibilitySettled`: When a flexibility payment settlement is completed
- `AdaptationParamsUpdated`: When adaptation policy (u_ref, o_ref, k_alpha, k_beta, window_size) is updated
- `AlphaBetaAdapted`: When alpha and beta are recalculated based on measurement windows

## Error Handling

The module includes comprehensive error handling for various scenarios:

Authorization & Role:
- `NotCustodian`
- `NotAllowedToManageProsumers`

Validation & Logic:
- `SameSenderReceiver`
- `InsufficientBalance`
- `PaymentTypeNotAllowed`
- `InvalidWindowSize` (adaptation policy)
- `EmptyMeasurements` (no samples provided)
- `MismatchedMeasurements` (length mismatch between under & over arrays)
- `MeasurementsExceedWindow` (length differs from configured window size)

Entity / Relationship:
- `SenderNotProsumer`, `ReceiverNotProsumer`
- `NotACommunity`
- `DifferentCommunities`

## Testing

The remuneration module includes comprehensive tests to verify its functionality and ensure correctness.

### Test Coverage

- **Administrative Operations**: Custodian, community, and prosumer management
- **Basic Payment Functionality**: Intra-community and inter-community payments
- **Balance Management**: Setting / updating balances
- **Parameter Management**: Alpha, beta, tolerance updates
- **Adaptive Policy**:
  - Setting adaptation params (success & failure paths)
  - Alpha/Beta adaptation (success math verification)
  - Validation errors (window size, empty, mismatched, length vs window)
  - Edge handling (negative factor clamp, overflow clamp)

### Flexibility Settlement Tests

1. Basic settlement (requested == delivered)
2. Under-delivery penalties
3. Over-delivery bonuses
4. Tolerance threshold behavior
5. Complex combined scenarios
6. Error handling (invalid actors, balances, types)
7. Inter-community flexibility settlements

### Adaptation Tests (Highlights)

- Proper storage of adaptation policy
- Adapting alpha/beta with deterministic expected outputs
- Rejecting incorrect measurement vector conditions
- Clamping to zero and to `u64::MAX` under extreme negative / overflow conditions

### Test Methodology

Each test:
1. Configures initial chain state (custodian, entities)
2. Applies parameter / adaptation configuration
3. Executes target extrinsic
4. Asserts resulting state (storage, balances, parameters)

Run tests:

```bash
cargo test -p remuneration
```

Parallel execution:

```bash
cargo test -p remuneration --jobs 6
```

## Integration

This module is designed to integrate with other pallets in the energy trading system:
- `orderbook_registry` for market participant management
- `trades_settlement` for concluding energy market transactions
- Standard Substrate pallets like `frame_system` and `pallet_balances`