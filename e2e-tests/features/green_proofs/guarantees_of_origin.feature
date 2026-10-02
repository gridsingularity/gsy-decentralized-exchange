Feature: Guarantees of origin over EWDS
  As a green-proof consumer
  I want to query local origin records for trades validated in a 15-minute window
  So that executed PV sales backed by export measurements are certified

  Background:
    Given the off-chain storage service is reachable

  Scenario: An executed PV sale backed by a seller export measurement is certified
    Given an executed PV sale of 1.234 kWh is seeded with a seller export of 3.0 kWh
    When the guarantees of origin around the verdict time are queried over EWDS until the seeded sale is certified
    Then exactly one local origin record is returned for the seeded sale
    And the record certifies 1.23 kWh of the seller facility for the seeded slot within its community
    And the REST endpoint returns the same records
    And the seeded green-proof documents are deleted

  Scenario: An executed PV sale without a seller export measurement is not certified
    Given an executed PV sale of 2.0 kWh is seeded without a seller export measurement
    When the guarantees of origin around the verdict time are queried over EWDS
    Then no local origin record is returned
    And the REST endpoint returns the same records
    And the seeded green-proof documents are deleted

