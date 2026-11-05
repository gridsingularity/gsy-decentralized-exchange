Feature: Canonical facility identities across the order lifecycle
  Background:
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts use facilities registered with uppercase UUIDs

  Scenario: UUID spelling does not change preferred partners or delivery verification
    When a bid at 20 preferring "bob" at 15 and an offer at 5 preferring "alice" at 12 are submitted
    Then the pair settles as "preferred" at 15 for pay-as-bid or max_offer 15, min_bid 15, midpoint 15 for pay-as-clear
    And the trade has "alice" as buyer and "bob" as seller
    And the trade is Matched before delivery verification
    When matching measurements use uppercase UUID references
    Then delivery verification marks the trade Executed and both orders Executed

  @ewds
  Scenario: FOS correlates canonical lifecycle IDs with mixed-format submissions
    Given FOS listens for order status notifications
    When orders with equivalent UUID and hex spellings are published on EWDS
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade has "alice" as buyer and "bob" as seller
    And FOS receives "submitted" notifications for both traded orders
    And FOS receives "filled" notifications for both traded orders
    When matching measurements use uppercase UUID references
    Then delivery verification marks the trade Executed and both orders Executed
    And FOS receives "executed" notifications for both traded orders
