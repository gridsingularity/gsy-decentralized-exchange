Feature: GSY DEX Partial Trade Execution
  As a user of the GSY DEX
  I want a residual order left by a partial match to be tradable without any further order submissions
  So that an over-sized bid is fully cleared across consecutive matching cycles from a single batch of orders

  Scenario: A residual left by a partial match is cleared in a later cycle from orders posted together
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When the community topology for a partial trade is submitted with one buyer and two sellers
    And the Market Orchestrator opens the partial-trade Spot market
    And "charlie" submits a bid of 10 energy and "bob" submits offers of 6 and 4 energy in the same cycle
    Then the matching engine settles a partial trade that leaves a residual bid
    And the residual bid is cleared by a later matching cycle and the trades add up to 10 energy
