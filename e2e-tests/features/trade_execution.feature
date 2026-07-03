Feature: GSY DEX Trade Execution
  As a user of the GSY DEX
  I want to submit a bid and an offer
  So that they are matched and a trade is executed

  Scenario: A simple bid and offer are matched and executed
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When the community topology and forecasts of 10 energy are submitted
    And the Market Orchestrator opens the Spot market for the next delivery slot
    And "charlie" submits a bid
    And "bob" submits an offer
    And measurements for "charlie" and "bob" assets are submitted
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the execution engine submits penalties for the trade

  Scenario: Multiple community markets run in parallel and execute trades
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When the community topologies and forecasts of 10 energy are submitted for communities "CommunityAlpha" and "CommunityBeta"
    And the Market Orchestrator opens the Spot markets for all communities
    And bids and offers are submitted for all communities
    And measurements for all community assets are submitted
    Then a trade is settled on-chain for each community market

  Scenario: Bids and offers must not match across community markets
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When two communities each submit multiple bids and offers selected to cross-match
    And the Market Orchestrator opens the Spot markets for the cross-matching communities
    And the cross-matching bids and offers are submitted for all communities
    Then every settled trade pairs a bid and an offer from the same community market
