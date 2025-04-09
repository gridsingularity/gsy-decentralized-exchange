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

### Flexibility Payment Calculation

The module includes a specialized settlement mechanism for flexibility services with:

- **Base Payment**: Calculated as minimum of requested and delivered flexibility multiplied by price
- **Under-delivery Penalties**: Applied when delivered flexibility is less than requested (beyond tolerance)
- **Over-delivery Bonuses**: Applied when delivered flexibility exceeds requested amount

Parameters for flexibility settlement:
- **Alpha**: Controls the penalty factor for under-delivery (fixed-point value)
- **Beta**: Controls the bonus factor for over-delivery (fixed-point value)
- **Tolerance**: Defines the acceptable deviation threshold without penalties (fixed-point value)

## Usage Examples

### Setting Up the System

```rust
// Set the custodian (privileged administrator)
Remuneration::update_custodian(origin, admin_account);

// Set parameters for flexibility settlement
Remuneration::update_alpha(origin, 500_000);     // 0.5 in fixed-point notation
Remuneration::update_beta(origin, 200_000);      // 0.2 in fixed-point notation
Remuneration::update_tolerance(origin, 100_000); // 0.1 in fixed-point notation

// Add a community
Remuneration::add_community(origin, community_account, dso_account, owner_account);

// Add prosumers to the community
Remuneration::add_prosumer(origin, prosumer_account, community_account);
```

### Processing Payments

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
```

## Flexibility Settlement Calculation

The settlement amount is calculated using the following formula:

    base_payment = min(requested, delivered) * price
    under_delivery_penalty = alpha * max(0, requested - delivered - threshold) * price
    over_delivery_bonus = beta * max(0, delivered - requested - threshold) * price

    final_amount = base_payment - under_delivery_penalty + over_delivery_bonus

Where:
- `threshold = tolerance * requested / 1_000_000`
- All parameters (alpha, beta, tolerance) use fixed-point arithmetic with 1.0 = 1,000,000

## Events

The module emits various events for tracking operations:

- `CustodianUpdated`: When the custodian is changed
- `CommunityAdded`/`CommunityRemoved`: When communities are added or removed
- `ProsumerAdded`/`ProsumerRemoved`: When prosumers are added or removed
- `PaymentAdded`: When a standard payment is processed
- `BalanceSet`: When an account's balance is manually updated by the custodian
- `AlphaUpdated`/`BetaUpdated`/`ToleranceUpdated`: When settlement parameters are changed
- `FlexibilitySettled`: When a flexibility payment settlement is completed

## Error Handling

The module includes comprehensive error handling for various scenarios:
- Authorization errors (e.g., `NotCustodian`, `NotAllowedToManageProsumers`)
- Validation errors (e.g., `SameSenderReceiver`, `InsufficientBalance`)
- Entity status errors (e.g., `SenderNotProsumer`, `ReceiverNotProsumer`)
- Relationship errors (e.g., `DifferentCommunities`, `NotACommunity`)
- Payment type errors (e.g., `PaymentTypeNotAllowed`)

## Testing

The remuneration module includes comprehensive tests to verify its functionality and ensure correctness:

### Test Coverage

- **Administrative Operations**: Tests for custodian management, community management, and prosumer management
- **Basic Payment Functionality**: Verification of intra-community and inter-community payments
- **Balance Management**: Tests for setting and updating balances
- **Parameter Management**: Tests for updating alpha, beta, and tolerance parameters

### Flexibility Settlement Tests

The module includes extensive testing for the flexibility settlement mechanism:

1. **Basic Settlement**: Verification of straightforward settlements where requested and delivered flexibility match
2. **Under-delivery Scenarios**: Tests to verify correct penalty calculation when delivered flexibility is less than requested
3. **Over-delivery Scenarios**: Tests to verify correct bonus calculation when delivered flexibility exceeds requested
4. **Tolerance Effects**: Tests to verify that deviations within tolerance thresholds do not incur penalties
5. **Complex Cases**: Tests for combinations of under/over-delivery and tolerance effects
6. **Error Handling**: Tests to ensure proper error responses for invalid inputs and states
7. **Inter-community Settlements**: Tests for flexibility settlements between different energy communities

### Test Methodology

Tests use the mock runtime environment to simulate a blockchain with the remuneration module. Each test:

1. Sets up the initial state (custodian, communities, prosumers, balances)
2. Configures settlement parameters (alpha, beta, tolerance) as needed
3. Executes the operation being tested
4. Verifies the resulting state changes (balance updates, event emissions)

This test suite ensures the module functions correctly under both normal operations and edge cases, providing confidence in the reliability and correctness of the implementation.

### Running Tests

Tests can be executed using the following command:

```bash
cargo test -p remuneration
```

For parallel test execution:

```bash
cargo test -p remuneration --jobs 6
```

## Integration

This module is designed to integrate with other pallets in the energy trading system:
- `orderbook_registry` for market participant management
- `trades_settlement` for concluding energy market transactions
- Standard Substrate pallets like `frame_system` and `pallet_balances`