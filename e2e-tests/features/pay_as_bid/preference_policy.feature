Feature: Preference settlement policy

  Scenario Outline: Preference policy - <case>
    Given the GSY DEX services are running
    And users "alice", "bob", and "charlie" are registered
    When the Market Orchestrator opens the Spot market for the next delivery slot
    And the community market and forecasts of 10 energy are submitted
    When a bid at <bid> preferring "<buyer_partner>" at <buyer_preferred> and an offer at <offer> preferring "<seller_partner>" at <seller_preferred> are submitted
    Then the pair settles as "<type>" at <pay_as_bid> for pay-as-bid or max_offer <max_offer>, min_bid <min_bid>, midpoint <midpoint> for pay-as-clear

    Examples:
      | case                         | bid | offer | buyer_partner | buyer_preferred | seller_partner | seller_preferred | type      | pay_as_bid | max_offer | min_bid | midpoint |
      | buyer-only preference        | 20  | 25    | bob           | 25              | none           | 0                | preferred | 25         | 25        | 25      | 25       |
      | seller-only preference       | 20  | 30    | none          | 0               | alice          | 20               | preferred | 20         | 20        | 20      | 20       |
      | reciprocal preferences       | 20  | 10    | bob           | 15              | alice          | 15               | preferred | 15         | 15        | 15      | 15       |
      | absent preferred rates       | 15  | 15    | bob           | 0               | alice          | 0                | preferred | 15         | 15        | 15      | 15       |
      | below normal offer limit     | 20  | 10    | bob           | 5               | alice          | 5                | preferred | 5          | 5         | 5       | 5        |
      | above normal bid limit       | 20  | 10    | bob           | 25              | alice          | 25               | preferred | 25         | 25        | 25      | 25       |
      | conflicting partners         | 20  | 10    | bob           | 15              | charlie        | 15               | standard  | 20         | 10        | 20      | 15       |
      | unavailable preferred seller | 20  | 10    | charlie       | 15              | none           | 0                | standard  | 20         | 10        | 20      | 15       |
      | unequal preferred rates      | 20  | 10    | bob           | 15              | alice          | 12               | standard  | 20         | 10        | 20      | 15       |
