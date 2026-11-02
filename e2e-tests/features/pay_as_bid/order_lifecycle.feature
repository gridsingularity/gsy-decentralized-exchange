Feature: Confirmed order lifecycle
  Scenario: Unmatched orders leave the book through cancellation
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    Then unmatched orders can be cancelled on-chain and in storage

  Scenario Outline: Full delivery completes verification after both facility measurements
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And "alice" submits a bid
    And "bob" submits an offer
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade is Matched before delivery verification
    When measurements of <first_energy> energy for only "<first>"'s facility are submitted
    And measurements of <second_energy> energy for only "<second>"'s facility are submitted
    Then delivery verification marks the trade and orders Executed without penalties

    Examples:
      | first | first_energy | second | second_energy |
      | alice | 10           | bob    | -10           |
      | bob   | -10          | alice  | 10            |
