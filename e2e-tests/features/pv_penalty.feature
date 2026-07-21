Feature: GSY DEX PV production shortfall penalty waterfall
  As the exchange operator
  I want a single PV asset's production shortfall to be waterfalled across its trades in
  time-priority order
  So that when one PV offer is matched into two trades, an under-production only penalizes the
  later of the two trades (the earlier commitment is honored first).

  Scenario: A single PV asset's production shortfall penalizes only the later of its two trades
    Given the GSY DEX services are running
    And users "bob" and "charlie" are registered and have collateral, with "alice" as the matching engine operator
    When a PV-penalty community topology with one PV asset and two meters is created for the next delivery slot
    And a single 5 kWh PV production offer forecast and two demand bid forecasts of 3 and 2 kWh are built
    And the Market Orchestrator opens the PV-penalty Spot market
    And the PV production offer and both demand bids are published
    Then two trades settle on the PV asset splitting its production into 3 and 2 kWh
    When a PV-asset production measurement of 4 kWh is submitted for the slot
    Then only the later of the two PV trades is penalized for the 1 kWh production shortfall
