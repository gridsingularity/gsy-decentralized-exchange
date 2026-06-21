Feature: GSY DEX Residual Trade Execution
  As a user of the GSY DEX
  I want a partially matched order to leave a residual order behind
  So that the residual energy can be traded in a following matching cycle

  Scenario: A partially matched bid leaves a residual that is traded later
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When the community topology for a residual trade is submitted with a bid of 10 energy and an offer of 6 energy
    And the Market Orchestrator opens the residual Spot market
    And "charlie" submits the residual-trade bid
    And "bob" submits the residual-trade offer
    Then the matching engine settles the initial trade of 6 energy and a residual bid for 4 energy remains
    When "bob" submits a follow-up offer of 4 energy for the residual bid
    Then the matching engine settles the residual trade of 4 energy
    And the settled trades for the market add up to the initial bid volume of 10 energy
