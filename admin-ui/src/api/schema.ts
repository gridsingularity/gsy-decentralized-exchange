// TypeScript models mirroring the Rust DB schemas in
// offchain-primitives/src/db_api_schema/*. Field names must match the JSON
// exactly. These are reused across the admin-ui milestones.
//
// Known footguns (from the admin-ui plan §9 — read carefully before joining):
//   1. A component's `area_uuid` does NOT hold a UUID: it literally holds the
//      asset's `area_hash` (orders.rs stores string_to_h256(area_hash)). So the
//      order/trade -> asset join key is `component.area_uuid == asset.area_hash`.
//   2. `energy_rate` is a TOTAL price for the order's energy (energy * rate),
//      NOT a per-kWh rate. A trade's `parameters.energy_rate` is copied verbatim
//      from the bid and is not rescaled to `selected_energy`, so it overstates
//      value on partial matches. Compute per-kWh as energy_rate / bid.energy.
//   3. time_slot unit split: trade/order `time_slot` is u64 while market
//      `time_slot` is u32. Both fit safely in a JS `number` (exact up to 2^53),
//      but never send a query time bound > u32::MAX (endpoints type bounds u32).

export type AssetType =
  | 'BATTERY'
  | 'SMART_METER'
  | 'PV'
  | 'GRID_METER'
  | 'EV'
  | 'HEAT_PUMP'
  | 'BOILER'
  | 'AREA';

export interface AreaTopologySchema {
  area_uuid: string;
  name: string;
  area_type: AssetType;
  area_hash: string;
}

export interface MarketTopologySchema {
  market_id: string;
  community_uuid: string;
  community_name: string;
  time_slot: number; // u32 seconds
  creation_time: number; // u32 seconds
  community_areas: AreaTopologySchema[];
}

// Summary of a community, derived by reducing all of its markets on the
// backend. `community_name` is the stable identity; `community_uuid` is
// randomized per market creation and is informational only, NOT a stable key.
export interface CommunitySummary {
  community_name: string;
  community_uuid: string;
  market_count: number; // u32
  earliest_slot: number; // u32 seconds
  latest_slot: number; // u32 seconds
}

export interface DbOrderComponent {
  // NB: `area_uuid` actually carries the asset's `area_hash` (footgun #1).
  area_uuid: string;
  market_id: string;
  time_slot: number; // u64 seconds
  creation_time: number; // u64 seconds
  energy: number;
  energy_rate: number; // total price, not per-kWh (footgun #2)
}

export interface DbBid {
  buyer: string;
  nonce: number;
  bid_component: DbOrderComponent;
}

export interface DbOffer {
  seller: string;
  nonce: number;
  offer_component: DbOrderComponent;
}

export type OrderStatus = 'Open' | 'Executed' | 'Expired' | 'Deleted';

// serde is configured with tag = "type", content = "data".
export type Order =
  | { type: 'Bid'; data: DbBid }
  | { type: 'Offer'; data: DbOffer };

export interface DbOrderSchema {
  _id: string;
  status: OrderStatus;
  order: Order;
}

export type TradeStatus = 'Settled' | 'Executed';

export interface TradeParameters {
  selected_energy: number;
  energy_rate: number; // total price, not per-kWh (footgun #2)
  trade_uuid: string;
}

export interface TradeSchema {
  _id: string;
  status: TradeStatus;
  seller: string; // AccountId32 string, NOT an area id
  buyer: string; // AccountId32 string, NOT an area id
  market_id: string;
  time_slot: number; // u64 seconds
  trade_uuid: string;
  creation_time: number; // u64 seconds
  offer: DbOffer;
  offer_hash: string;
  bid: DbBid;
  bid_hash: string;
  residual_offer: DbOffer | null;
  residual_bid: DbBid | null;
  parameters: TradeParameters;
}

// Returned by GET /trades-canonical: a TradeSchema (all fields flattened at the
// top level, including the raw `seller`/`buyer` account ids) enriched with the
// human-readable asset names resolved from the topology `area_hash` -> `name`
// map. `seller_name`/`buyer_name` are null when the component `area_uuid`
// (which carries the asset's `area_hash`, footgun #1) cannot be resolved.
export interface TradeCanonicalSchema extends TradeSchema {
  seller_name: string | null;
  buyer_name: string | null;
}

export interface MeasurementSchema {
  // area_uuid here carries the asset `name` (sensor id), area_hash the per-market
  // random hash — see plan §9.4 before joining.
  area_uuid: string;
  area_hash: string;
  community_uuid: string;
  time_slot: number; // u64 seconds
  creation_time: number;
  energy_kwh: number;
}

export interface ForecastSchema extends MeasurementSchema {
  confidence: number;
}

export interface TimeSeriesPoint {
  timestamp: number;
  value: number;
}

export interface TradedEnergyResponse {
  id: string;
  traded_energy: TimeSeriesPoint[];
}
