@ewds
Feature: Events over EWDS
  As another system connected to EWDS
  I want to send sites, facilities and measurements to GSY as EWDS events
  So that the off-chain storage stores them without a request/reply round trip

  Scenario: Sites, facilities and measurements published on EWDS are stored
    Given the GSY DEX services are running
    When a site, a facility and a measurement batch are published on EWDS
    Then the off-chain storage stores the site and the facility
    And the off-chain storage stores the measurement batch

  Scenario Outline: Orders published on EWDS produce a verified <verdict> trade
    Given the GSY DEX services are running
    And FOS listens for order status notifications
    And FOS listens for trade status notifications
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    And a bid by "alice" and an offer by "bob" are published on EWDS
    Then the matching engine matches the bid and offer and a trade is settled on-chain
    And the trade has "alice" as buyer and "bob" as seller
    And the trade is Matched before delivery verification
    And FOS receives the Matched trade notification
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
