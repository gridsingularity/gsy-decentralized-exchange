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

  Scenario: Orders published on EWDS are placed on-chain and matched
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" the matching engine operator are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    And a bid by "alice" and an offer by "bob" are published on EWDS
    Then the matching engine matches the bid and offer and a trade is settled on-chain
