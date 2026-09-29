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
