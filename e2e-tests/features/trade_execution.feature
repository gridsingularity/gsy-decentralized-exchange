Feature: GSY DEX Trade Execution
  As a user of the GSY DEX
  I want to submit a bid and an offer
  So that they are matched and a trade is executed

  Scenario: A simple bid and offer are matched and executed
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered and have collateral
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community topology and forecasts of 10 energy are submitted
    And "alice" submits a bid
    And "bob" submits an offer
    And measurements for "alice" and "bob" assets are submitted
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the execution engine submits penalties for the trade
