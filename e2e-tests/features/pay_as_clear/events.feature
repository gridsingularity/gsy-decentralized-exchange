@ewds
Feature: Order lifecycle notifications over EWDS
  Scenario: Submissions and status notifications share the order topic safely
    Given the GSY DEX services are running
    And FOS listens for order status notifications
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And a bid by "alice" and an offer by "bob" are published on EWDS
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade has "alice" as buyer and "bob" as seller
    And FOS receives "submitted" notifications for both traded orders
    And FOS receives "filled" notifications for both traded orders
    When measurements of 2 energy for facilities are submitted
    Then delivery verification marks the trade and orders Executed without penalties
    And FOS receives "executed" notifications for both traded orders
    And FOS receives a rejection for an order in a closed market without storing it
