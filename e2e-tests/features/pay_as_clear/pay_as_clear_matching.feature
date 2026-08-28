Feature: Two-Sided Pay-as-Clear Matching
  As a market participant
  I want accepted merit-order trades to share one clearing price
  So that the market clears at the marginal accepted offer rate

  Scenario: Accepted bids and offers settle at a uniform clearing price
    Given the GSY DEX services are running
    And the matching engine uses "pay_as_clear"
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And measurements for facilities are submitted
    And the pay-as-clear order book is submitted
    Then the market clears 7 energy at a uniform price of 10
    And the matching engine matches the bid and offer and a trade is settled on-chain
    And orders beyond the clearing point remain open
    And the execution engine submits penalties for the trade

  Scenario: A preferred bilateral trade is priced separately from the standard clearing market
    Given the GSY DEX services are running
    And the matching engine uses "pay_as_clear"
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And measurements for facilities are submitted
    And a preferred bilateral pair and standard pay-as-clear order book are submitted
    Then the preferred bilateral trade clears 2 energy at a negotiated price of 11
    And the remaining standard market clears 7 energy at a uniform price of 10
    And the matching engine matches the bid and offer and a trade is settled on-chain
    And orders beyond the clearing point remain open
    And the execution engine submits penalties for the trade
