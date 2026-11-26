Feature: Preference matching policy

  Scenario Outline: Preference policy - <case> (<algorithm>)
    Given the GSY DEX services are running
    And the matching engine uses "<algorithm>"
    And the DEX service accounts are authorized
    When the Market Orchestrator opens the Spot market for the current delivery slot
    And forecasts of 10 energy are submitted
    When a bid at <bid> preferring "<buyer_partner>" at <buyer_preferred> and an offer at <offer> preferring "<seller_partner>" at <seller_preferred> are submitted
    Then the pair is matched as "<type>" at <pay_as_bid> for pay-as-bid or max_offer <max_offer>, min_bid <min_bid>, midpoint <midpoint> for pay-as-clear

    Examples: Pay-as-bid
      | case                         | algorithm    | bid | offer | buyer_partner | buyer_preferred | seller_partner | seller_preferred | type      | pay_as_bid | max_offer | min_bid | midpoint |
      | buyer-only preference        | pay_as_bid   | 20  | 25    | bob           | 25              | none           | 0                | preferred | 25         | 25        | 25      | 25       |
      | seller-only preference       | pay_as_bid   | 20  | 30    | none          | 0               | alice          | 20               | preferred | 20         | 20        | 20      | 20       |
      | reciprocal unequal rates     | pay_as_bid   | 20  | 10    | bob           | 15              | alice          | 12               | preferred | 15         | 15        | 15      | 15       |
      | absent preferred rates       | pay_as_bid   | 15  | 15    | bob           | 0               | alice          | 0                | preferred | 15         | 15        | 15      | 15       |
      | below normal offer limit     | pay_as_bid   | 20  | 10    | bob           | 5               | alice          | 5                | preferred | 5          | 5         | 5       | 5        |
      | above normal bid limit       | pay_as_bid   | 20  | 10    | bob           | 25              | alice          | 25               | preferred | 25         | 25        | 25      | 25       |
      | conflicting partners         | pay_as_bid   | 20  | 10    | bob           | 15              | charlie        | 15               | standard  | 20         | 20        | 20      | 20       |
      | unavailable preferred seller | pay_as_bid   | 20  | 10    | charlie       | 15              | none           | 0                | standard  | 20         | 20        | 20      | 20       |
      | incompatible preferred rates | pay_as_bid   | 20  | 10    | bob           | 9               | alice          | 12               | standard  | 20         | 20        | 20      | 20       |

    Examples: Pay-as-clear
      | case                         | algorithm    | bid | offer | buyer_partner | buyer_preferred | seller_partner | seller_preferred | type      | pay_as_bid | max_offer | min_bid | midpoint |
      | buyer-only preference        | pay_as_clear | 20  | 25    | bob           | 25              | none           | 0                | preferred | 25         | 25        | 25      | 25       |
      | seller-only preference       | pay_as_clear | 20  | 30    | none          | 0               | alice          | 20               | preferred | 20         | 20        | 20      | 20       |
      | reciprocal unequal rates     | pay_as_clear | 20  | 10    | bob           | 15              | alice          | 12               | preferred | 15         | 15        | 15      | 15       |
      | absent preferred rates       | pay_as_clear | 15  | 15    | bob           | 0               | alice          | 0                | preferred | 15         | 15        | 15      | 15       |
      | below normal offer limit     | pay_as_clear | 20  | 10    | bob           | 5               | alice          | 5                | preferred | 5          | 5         | 5       | 5        |
      | above normal bid limit       | pay_as_clear | 20  | 10    | bob           | 25              | alice          | 25               | preferred | 25         | 25        | 25      | 25       |
      | conflicting partners         | pay_as_clear | 20  | 10    | bob           | 15              | charlie        | 15               | standard  | 20         | 20        | 20      | 20       |
      | unavailable preferred seller | pay_as_clear | 20  | 10    | charlie       | 15              | none           | 0                | standard  | 20         | 20        | 20      | 20       |
      | incompatible preferred rates | pay_as_clear | 20  | 10    | bob           | 9               | alice          | 12               | standard  | 20         | 20        | 20      | 20       |
