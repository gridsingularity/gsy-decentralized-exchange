Feature: Community-Aware Market Orchestration
  As a DEX operator
  I want each community to receive its own market
  So that orders from different communities cannot share a market

  Scenario: Two communities trade only within their own Spot markets
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When two communities are submitted to off-chain storage
    Then the Market Orchestrator opens a distinct Spot market for each community
    When compatible orders are submitted to different community markets
    Then no cross-community trade is settled
    When matching counterpart orders are submitted within both community markets
    Then each community market settles only its own bid and offer
    And measurements for both community markets are submitted
    And the execution engine submits penalties for the trade
