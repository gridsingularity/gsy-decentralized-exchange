Feature: GSY DEX PV and Demand Forecasting
  As the exchange operator
  I want PV-production and demand forecasts to flow through the community client pipeline
  So that a PV asset becomes a confidence-priced production offer and a metered load becomes a
  consumption bid, and their production nets correctly in the inter-community market.

  Scenario: A single community ingests PV and demand forecasts into an offer and a bid and settles a trade
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When a PV-and-demand community topology is created for the next delivery slot
    And a PV forecaster response is ingested into a production offer forecast
    And a demand forecast is constructed for the consumption meter
    And the PV and demand forecasts are validated and forwarded to offchain storage
    And the Market Orchestrator opens the PV-and-demand Spot market
    And the PV production offer and the demand bid are published
    Then the PV forecast is stored as an offer with a confidence-lifted rate floor and the demand forecast as a flat-rate bid
    And a trade settles between the PV offer and the demand bid on-chain

  Scenario: Two communities net to a bid and an offer with PV production included in the net
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When two eligible communities each ingest PV and demand forecasts that net to a bid and an offer
    And the inter-community PV-and-demand forecasts are forwarded and read back from offchain storage
    Then each community's aggregated net import reflects its PV production and demand
    And the aggregated inter-community orders reflect the per-community nets
