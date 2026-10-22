Feature: Confirmed order lifecycle
  Scenario: Unmatched orders leave the book through cancellation
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    Then unmatched orders can be cancelled on-chain and in storage

  Scenario: Full delivery completes verification without a penalty
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And "alice" submits a bid
    And "bob" submits an offer
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade is Settled before delivery verification
    When measurements of 10 energy for facilities are submitted
    Then delivery verification marks the trade and orders Executed without penalties
