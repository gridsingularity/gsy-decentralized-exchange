Feature: GSY DEX Community Client
  As a user of the GSY DEX
  I want to read the FEDECOM ontology data
  So that the community members, assets and markets are created

  Scenario: Ontology data are saved to GSY DEX Offchain Storage
    Given the GSY DEX services are running
    When the GSY DEX Community Client reads the FEDECOM ontology data
    Then the ontology data are saved to GSY DEX offchain storage
