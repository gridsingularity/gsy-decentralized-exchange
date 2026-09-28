// Asset-identity resolution for the Asset view (Milestone 5).
//
// CRITICAL data-model note (see admin-ui plan §9): a physical asset's
// `area_uuid`/`area_hash` are randomized PER MARKET, so the same asset carries
// a DIFFERENT `area_hash` in every slot's market. The only stable identity
// across slots is the asset `name` (+ its `area_type`). This module resolves a
// stable `name` into the per-slot `{ time_slot, market_id, area_hash }` map that
// every downstream fetch/filter is keyed on.
//
// Join keys downstream (footgun #1 — component `area_uuid` holds the asset's
// `area_hash`):
//   order:  order.data.{bid,offer}_component.area_uuid === slot.area_hash
//   trade:  trade.{offer.offer_component,bid.bid_component}.area_uuid
//              === slot.area_hash  (of the trade's market)

import type {
  AssetType,
  DbOrderSchema,
  MarketTopologySchema,
  OrderStatus,
  TradeCanonicalSchema,
  TradeStatus,
} from '../api/schema';
import { perKwhFromComponent, perKwhRate } from './aggregate';

/** One per-slot identity of an asset, resolved from a single market. */
export interface AssetSlot {
  time_slot: number; // u32 seconds (market slot)
  market_id: string;
  area_hash: string; // per-market-random; the order/trade join key
  area_uuid: string; // topology area_uuid (also per-market-random)
  area_type: AssetType;
}

/**
 * Resolve a stable asset `name` into its per-slot identities across a
 * community's markets. For each market whose `community_areas` contains an area
 * with this exact `name`, emit one `AssetSlot`. Result is sorted by time_slot
 * descending (newest first), matching the market list ordering.
 */
export function resolveAssetSlots(
  markets: MarketTopologySchema[],
  assetName: string,
): AssetSlot[] {
  const out: AssetSlot[] = [];
  for (const m of markets) {
    const area = m.community_areas.find((a) => a.name === assetName);
    if (!area) continue;
    out.push({
      time_slot: m.time_slot,
      market_id: m.market_id,
      area_hash: area.area_hash,
      area_uuid: area.area_uuid,
      area_type: area.area_type,
    });
  }
  return out.sort((a, b) => b.time_slot - a.time_slot);
}

// --- Orders panel rows ----------------------------------------------------

export interface AssetOrderRow {
  id: string;
  time_slot: number;
  side: 'Bid' | 'Offer';
  energy: number;
  /** The component's TOTAL price (footgun #2), labelled "bid/offer total". */
  energyRateTotal: number;
  perKwh: number | null;
  status: OrderStatus;
}

/**
 * Build order rows for a single market's fetched orders, keeping only orders
 * whose bid/offer component `area_uuid` equals that market's `area_hash`.
 */
export function assetOrderRows(
  orders: DbOrderSchema[],
  slot: AssetSlot,
): AssetOrderRow[] {
  const rows: AssetOrderRow[] = [];
  for (const o of orders) {
    const comp =
      o.order.type === 'Bid'
        ? o.order.data.bid_component
        : o.order.data.offer_component;
    if (comp.area_uuid !== slot.area_hash) continue;
    rows.push({
      id: o._id,
      time_slot: slot.time_slot,
      side: o.order.type,
      energy: comp.energy,
      energyRateTotal: comp.energy_rate,
      perKwh: perKwhFromComponent(comp),
      status: o.status,
    });
  }
  return rows;
}

// --- Trades panel rows ----------------------------------------------------

export interface AssetTradeRow {
  trade_uuid: string;
  time_slot: number;
  /** Seller if the asset was the offer side, Buyer if the bid side. */
  role: 'Seller' | 'Buyer';
  /** The OTHER side's account key (truncated in the UI). */
  counterparty: string;
  /** The OTHER side's resolved asset name, or null when unresolvable. */
  counterpartyName: string | null;
  selected_energy: number;
  perKwh: number | null;
  status: TradeStatus;
}

/**
 * Filter window trades to those in which this asset participated (offer or bid
 * side matched one of its per-slot `area_hash`es) and build display rows.
 * The asset's account keys never appear in `trade.seller`/`trade.buyer` joins —
 * those are account ids used only as the counterparty label.
 */
export function assetTradeRows(
  trades: TradeCanonicalSchema[],
  slots: AssetSlot[],
): AssetTradeRow[] {
  const hashes = new Set(slots.map((s) => s.area_hash));
  const rows: AssetTradeRow[] = [];
  for (const t of trades) {
    const isSeller = hashes.has(t.offer.offer_component.area_uuid);
    const isBuyer = hashes.has(t.bid.bid_component.area_uuid);
    if (!isSeller && !isBuyer) continue;
    // Prefer the offer side when (unexpectedly) both match.
    const role: 'Seller' | 'Buyer' = isSeller ? 'Seller' : 'Buyer';
    rows.push({
      trade_uuid: t.trade_uuid,
      time_slot: t.time_slot,
      role,
      // When this asset is the Seller, the counterparty is the buyer side.
      counterparty: role === 'Seller' ? t.buyer : t.seller,
      counterpartyName: role === 'Seller' ? t.buyer_name : t.seller_name,
      selected_energy: t.parameters.selected_energy,
      perKwh: perKwhRate(t),
      status: t.status,
    });
  }
  return rows;
}

export interface TradedEnergyPoint {
  time_slot: number;
  value: number; // Σ selected_energy in the slot (kWh)
}

/**
 * Sum `selected_energy` per `time_slot` across the asset's filtered trade rows.
 * Equivalent to the server's `/traded-energy?id=<area_hash>` per slot, derived
 * client-side to avoid one call per per-slot area_hash.
 */
export function tradedEnergySeries(rows: AssetTradeRow[]): TradedEnergyPoint[] {
  const bySlot = new Map<number, number>();
  for (const r of rows) {
    bySlot.set(r.time_slot, (bySlot.get(r.time_slot) ?? 0) + r.selected_energy);
  }
  return [...bySlot.entries()]
    .map(([time_slot, value]) => ({ time_slot, value }))
    .sort((a, b) => a.time_slot - b.time_slot);
}
