Feature: GSY DEX metering-point verdicts
  As the exchange operator
  I want every trade side judged against the metering-point measurement of its building
  So that a building that delivered what it traded is executed and certified, an over-consuming
  building is charged for its excess, a building without a measurement is charged in full, and a
  building that under-delivers charges its sellers first.

  # The measurement rows are posted one at a time, in table order, and the execution engine gives
  # a trade its verdict on the first cycle that sees a row covering one of its sides. Each table
  # is ordered so that the rows deciding a verdict are stored before any row that, seen on its
  # own, would judge the same trade differently: the buyer buildings before the seller building
  # in the first scenario, the seller building before the buyer building in the second.
  #
  # A `seller_area` cell may list several areas, meaning the trade is sold by one of them; the
  # trades sharing such a cell must each be sold by a different one of its areas.

  Scenario: A metering point executes a delivered trade and penalizes over-consumption and a missing measurement
    # Three 1 kWh offers against three 1 kWh bids: every pairing clears whole orders, so no trade
    # depends on matching an offer's residual. Which PV sells to which buyer is up to the matcher.
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When a metering-point community "MeteringPointCommunity" is created for the next delivery slot with areas:
      | area     | type        |
      | MP_PV1   | PV          |
      | MP_PV2   | PV          |
      | MP_PV3   | PV          |
      | MP_SM    | SMART_METER |
      | MP_B1_SM | SMART_METER |
      | MP_B2_SM | SMART_METER |
      | MP_B3_SM | SMART_METER |
    And these metering-point orders are built:
      | user    | side  | area     | energy_kwh |
      | bob     | offer | MP_PV1   | 1.0        |
      | bob     | offer | MP_PV2   | 1.0        |
      | bob     | offer | MP_PV3   | 1.0        |
      | charlie | bid   | MP_B1_SM | 1.0        |
      | charlie | bid   | MP_B2_SM | 1.0        |
      | charlie | bid   | MP_B3_SM | 1.0        |
    And the Market Orchestrator opens the metering-point Spot market
    And the metering-point offers and bids are published
    Then the metering-point market settles these trades:
      | seller_area            | buyer_area | energy_kwh |
      | MP_PV1, MP_PV2, MP_PV3 | MP_B1_SM   | 1.0        |
      | MP_PV1, MP_PV2, MP_PV3 | MP_B2_SM   | 1.0        |
      | MP_PV1, MP_PV2, MP_PV3 | MP_B3_SM   | 1.0        |
    When these metering-point measurements are submitted for the slot:
      | metering_point | members                       | completeness | energy_kwh | missing_meters |
      | MPBuyerHouse1  | MP_B1_SM                      | complete     | 1.0        |                |
      | MPBuyerHouse2  | MP_B2_SM                      | complete     | 1.5        |                |
      | MPBuyerHouse3  | MP_B3_SM                      | missing      | 0.0        | B3             |
      | MPSellerHouse  | MP_PV1, MP_PV2, MP_PV3, MP_SM | complete     | -3.0       |                |
    Then the execution engine gives these metering-point verdicts on-chain:
      | trade_bought_by | verdict   | penalized_user | penalty_energy |
      | MP_B1_SM        | executed  |                |                |
      | MP_B2_SM        | penalized | charlie        | 500            |
      | MP_B3_SM        | penalized | charlie        | 1000           |
    And the metering-point trades are marked in the offchain storage:
      | trade_bought_by | status    |
      | MP_B1_SM        | Executed  |
      | MP_B2_SM        | Penalized |
      | MP_B3_SM        | Penalized |
    And the offchain storage certifies only the metering-point trade bought by "MP_B1_SM", for 1.0 kWh from metering point "MPSellerHouse"

  Scenario: A seller building's under-delivery is charged to its seller before its buyers
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When a metering-point community "MeteringPointSellerCommunity" is created for the next delivery slot with areas:
      | area    | type        |
      | MS_PV   | PV          |
      | MS_B_SM | SMART_METER |
    And these metering-point orders are built:
      | user    | side  | area    | energy_kwh |
      | bob     | offer | MS_PV   | 4.0        |
      | charlie | bid   | MS_B_SM | 4.0        |
    And the Market Orchestrator opens the metering-point Spot market
    And the metering-point offers and bids are published
    Then the metering-point market settles these trades:
      | seller_area | buyer_area | energy_kwh |
      | MS_PV       | MS_B_SM    | 4.0        |
    When these metering-point measurements are submitted for the slot:
      | metering_point | members | completeness | energy_kwh | missing_meters |
      | MSSellerHouse  | MS_PV   | complete     | -3.0       |                |
      | MSBuyerHouse   | MS_B_SM | complete     | 4.0        |                |
    Then the execution engine gives these metering-point verdicts on-chain:
      | trade_bought_by | verdict   | penalized_user | penalty_energy |
      | MS_B_SM         | penalized | bob            | 1000           |
    And the metering-point trades are marked in the offchain storage:
      | trade_bought_by | status    |
      | MS_B_SM         | Penalized |
    And the offchain storage certifies none of the metering-point trades
