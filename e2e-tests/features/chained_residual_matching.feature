Feature: GSY DEX chained remainder matching
  As a user of the GSY DEX
  I want an order larger than any single counter-order to be cleared by several of them
  So that each partial match leaves a remainder that the next matching cycle clears in turn

  # One large order against three smaller ones on the other side. The matcher clears one pair per
  # order per cycle, so the large order settles as a chain: the first trade leaves a remainder,
  # the second trade is made from that remainder and leaves another, and the third trade is made
  # from the second remainder. Whatever the order in which the small orders are picked, the large
  # order always covers what is left of them, so every trade clears one small order exactly.
  # No measurements are submitted: only the settlement of the chain is checked.
  #
  # Rates: an order is priced at its energy times the rate, and a remainder keeps its parent's
  # price. The 6 kWh offer (0.07) is priced 0.42, so each bid of the offer chain must be priced at
  # least that: at 0.5 per kWh the 1 kWh bid is 0.5. At the default 0.3 it would be 0.3 and
  # would, correctly, never match the second remainder.
  #
  # Run just these with `--name "chained remainder"`.

  Background:
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator

  Scenario: A chained remainder of an offer clears three bids
    When a metering-point community "ChainedRemainderOfferCommunity" is created for the next delivery slot with areas:
      | area     | type        |
      | CO_PV    | PV          |
      | CO_B1_SM | SMART_METER |
      | CO_B2_SM | SMART_METER |
      | CO_B3_SM | SMART_METER |
    And these metering-point orders are built:
      | user    | side  | area     | energy_kwh |
      | bob     | offer | CO_PV    | 6.0        |
      | charlie | bid   | CO_B1_SM | 3.0        |
      | charlie | bid   | CO_B2_SM | 2.0        |
      | charlie | bid   | CO_B3_SM | 1.0        |
    And the metering-point bids are priced at 0.5 per kWh
    And the Market Orchestrator opens the metering-point Spot market
    And the metering-point offers and bids are published
    Then the metering-point market settles these trades:
      | seller_area | buyer_area | energy_kwh |
      | CO_PV       | CO_B1_SM   | 3.0        |
      | CO_PV       | CO_B2_SM   | 2.0        |
      | CO_PV       | CO_B3_SM   | 1.0        |

  Scenario: A chained remainder of a bid clears three offers
    # The mirror image: all three trades are bought by the same area, so they are identified by
    # their seller area.
    When a metering-point community "ChainedRemainderBidCommunity" is created for the next delivery slot with areas:
      | area    | type        |
      | CB_PV1  | PV          |
      | CB_PV2  | PV          |
      | CB_PV3  | PV          |
      | CB_B_SM | SMART_METER |
    And these metering-point orders are built:
      | user    | side  | area    | energy_kwh |
      | bob     | offer | CB_PV1  | 3.0        |
      | bob     | offer | CB_PV2  | 2.0        |
      | bob     | offer | CB_PV3  | 1.0        |
      | charlie | bid   | CB_B_SM | 6.0        |
    And the Market Orchestrator opens the metering-point Spot market
    And the metering-point offers and bids are published
    Then the metering-point market settles these trades, one per seller area:
      | seller_area | buyer_area | energy_kwh |
      | CB_PV1      | CB_B_SM    | 3.0        |
      | CB_PV2      | CB_B_SM    | 2.0        |
      | CB_PV3      | CB_B_SM    | 1.0        |
