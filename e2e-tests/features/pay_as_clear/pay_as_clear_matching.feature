Feature: Two-Sided Pay-as-Clear Matching
  As a market participant
  I want accepted merit-order trades to share one clearing price
  So that the market applies its configured pricing policy and scarcity rules

  Scenario: Accepted bids and offers settle at a uniform clearing price
    Given the GSY DEX services are running
    And the matching engine uses "pay_as_clear"
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And measurements for facilities are submitted
    And the pay-as-clear order book is submitted
    Then the standard market clears 7 energy at the configured price: max_offer 10, min_bid 17, midpoint 13.5
    And the matching engine matches the bid and offer and a trade is settled on-chain
    And unmatched orders remain open on-chain and in storage
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
    And the standard market clears 7 energy at the configured price: max_offer 10, min_bid 17, midpoint 13.5
    And the matching engine matches the bid and offer and a trade is settled on-chain
    And unmatched orders remain open on-chain and in storage
    And the execution engine submits penalties for the trade

  Scenario Outline: Clearing with <book> uses the appropriate price
    Given the GSY DEX services are running
    And the matching engine uses "pay_as_clear"
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And measurements for facilities are submitted
    And the pay-as-clear order book with "<book>" is submitted
    Then the standard market clears 7 energy at the configured price: max_offer <max_offer>, min_bid <min_bid>, midpoint <midpoint>
    And the matching engine matches the bid and offer and a trade is settled on-chain
    And unmatched orders remain open on-chain and in storage
    And the execution engine submits penalties for the trade

    Examples:
      | book                    | max_offer | min_bid | midpoint |
      | supply scarcity         | 17        | 17      | 17       |
      | demand scarcity         | 10        | 10      | 10       |
      | simultaneous exhaustion | 10        | 17      | 13.5     |
