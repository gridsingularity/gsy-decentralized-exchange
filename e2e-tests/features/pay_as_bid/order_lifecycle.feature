Feature: Confirmed order lifecycle
  Scenario: Unmatched orders leave the book through cancellation
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    Then unmatched orders can be cancelled on-chain and in storage

  Scenario Outline: Full delivery completes verification using only <participant>'s facility
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And "alice" submits a bid
    And "bob" submits an offer
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade is Matched before delivery verification
    When measurements of 10 energy for only "<participant>"'s facility are submitted
    Then delivery verification marks the trade and orders Executed without penalties

    Examples:
      | participant |
      | alice       |
      | bob         |
