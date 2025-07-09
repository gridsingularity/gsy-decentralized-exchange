Feature: GSY DEX Trade Execution
  As a user of the GSY DEX
  I want to submit a bid and an offer
  So that they are matched and a trade is executed

  Scenario: A simple bid and offer are matched and executed
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered and have collateral
    When "alice" submits a bid for 100 energy at a rate of 15
    And "bob" submits an offer for 100 energy at a rate of 10
    And measurements for "alice" and "bob" assets are submitted
    Then the matching engine matches the bid and offer and a trade is settled on-chain