@ewds
Feature: Order lifecycle notifications over EWDS
  Scenario Outline: Submissions and notifications produce a verified <verdict> trade
    Given the GSY DEX services are running
    And FOS listens for order status notifications
    And FOS listens for trade status notifications
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And a bid by "alice" and an offer by "bob" are published on EWDS
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade has "alice" as buyer and "bob" as seller
    And the trade is Settled before delivery verification
    And FOS receives the Matched trade notification
    And FOS receives the Settled trade notification
    And FOS receives "submitted" notifications for both traded orders
    And FOS receives "filled" notifications for both traded orders
    When measurements of <energy> energy for facilities are submitted
    Then delivery verification marks the trade <verdict> and both orders Executed
    And FOS receives the <verdict> trade notification
    And FOS receives "executed" notifications for both traded orders
    And FOS receives a rejection for an order in a closed market without storing it

    Examples:
      | energy | verdict   |
      | 2      | Executed  |
      | 1      | Penalized |
