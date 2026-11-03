Feature: Delivery deviation penalties
  # The E2E engine uses a 10% penalty rate and the default 5% tolerance.
  # Penalties are contract units (10,000 units per whole penalty amount).
  Scenario Outline: Both facility deviations determine the delivery verdict
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And "alice" submits a bid
    And "bob" submits an offer
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade is Matched before delivery verification
    When measurements of <buyer_energy> energy for only "alice"'s facility are submitted
    And measurements of <seller_energy> energy for only "bob"'s facility are submitted
    Then delivery verification marks the trade <status> and both orders Executed
    And the trade records exactly <buyer_penalty> buyer and <seller_penalty> seller penalty units

    Examples:
      | buyer_energy | seller_energy | status    | buyer_penalty | seller_penalty |
      | 10.5         | -9.5          | Executed  | 0             | 0              |
      | 11           | -9            | Penalized | 1000          | 1000           |
      | 9            | -11           | Penalized | 1000          | 1000           |
