Feature: Preference-Based Matching with Dedicated Pricing

  Scenario: A bilateral preferred partner trade is matched with its special price
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" are registered and have collateral
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community topology and forecasts of 100 energy are submitted by "alice", "bob", and "charlie"

    When "alice" submits a bid for 100 energy at a normal rate of 15, with a preferred rate of 12 for partner "bob"
    And "bob" submits an offer for 150 energy at a normal rate of 10, with a preferred rate of 12 for partner "alice"
    And "charlie" submits a cheaper open-market offer for 100 energy at a rate of 9

    Then a trade is settled on-chain between "alice" and "bob" for 100 energy
    And the trade price is exactly 12, matching the preferred rate
    And Bob's residual offer of 50 energy is available for the next matching phase
    And Charlie's cheaper offer remains untouched in this phase
