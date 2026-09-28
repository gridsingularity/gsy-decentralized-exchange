Feature: GSY DEX Residual Order Matching
  As a user of the GSY DEX
  I want an order that is only partially matched to leave a residual order behind
  So that the remaining energy is cleared in a following matching cycle

  Background:
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator

  Scenario: A residual left by a partial match is cleared by a order posted in a following tick
    When the residual community topology is submitted with a bid of 10 energy and an offer of 6 energy
    And the Market Orchestrator opens the residual-matching Spot market
    And "charlie" submits the residual-trade bid
    And "bob" submits the residual-trade offer
    Then the matching engine settles the initial trade of 6 energy and a residual bid for 4 energy remains
    When "bob" submits a follow-up offer of 4 energy for the residual bid
    Then the matching engine settles the residual trade of 4 energy
    And the settled trades for the market add up to the initial bid volume of 10 energy

  Scenario: A residual left by a partial match is cleared from orders posted in the same batch
    When the partial community topology is submitted with one buyer and two sellers
    And the Market Orchestrator opens the residual-matching Spot market
    And "charlie" submits a bid of 10 energy and "bob" submits offers of 6 and 4 energy in the same cycle
    Then the matching engine settles a partial trade that leaves a residual bid
    And the residual bid is cleared by a later matching cycle and the trades add up to 10 energy
