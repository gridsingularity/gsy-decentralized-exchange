Feature: GSY DEX Inter-Community Market
  As the exchange operator
  I want eligible communities to trade their aggregated net energy in a single shared market
  So that a community that is net-short buys from a community that is net-long,
  while each community's per-community spot market keeps running independently.

  Scenario: Two communities net to a bid and an offer and settle in the shared inter-community market
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When the inter-community market is created for the next delivery slot
    And two eligible communities submit forecasts that net to a bid and an offer
    And the Market Orchestrator opens the inter-community market
    And the aggregated inter-community orders are published
    And measurements for the inter-community community assets are submitted
    Then exactly one aggregated order per community is stored in the inter-community market
    And a trade is settled in the inter-community market with the reserved market id
    And no inter-community order cross-matches a spot order
