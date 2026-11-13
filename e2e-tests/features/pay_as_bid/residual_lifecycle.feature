Feature: Preferred residual settlement lifecycle

  Scenario Outline: A residual <side> is consumed in a <consumption> settlement
    Given the GSY DEX services are running
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 100 energy are submitted by "alice", "bob", and "charlie"
    Then the preferred <side> residual lifecycle succeeds with "<consumption>" consumption

    Examples:
      | side  | consumption |
      | bid   | same-batch  |
      | offer | same-batch  |
      | bid   | later-cycle |
      | offer | later-cycle |
