// Pure aggregation helpers for the Market view (Milestone 4).
//
// CRITICAL data-model note (admin-ui plan §9.3, schema footgun #2):
//   `trade.parameters.energy_rate` is a TOTAL price copied verbatim from the
//   bid and is NOT rescaled to `selected_energy`. To recover a per-kWh rate we
//   must divide the bid's total price by the bid's energy, NOT by the traded
//   (selected) energy. Concretely, a trade with:
//       parameters.energy_rate = 2.1, bid.energy = 7.0, selected_energy = 6.0
//   must yield perKwhRate = 2.1 / 7.0 = 0.3  (NOT 2.1), and contributes 6.0 to
//   the traded volume for its slot.

import type { DbOrderComponent, TradeSchema } from '../api/schema';

/**
 * Per-kWh rate for a single order component, derived from its TOTAL price
 * (`energy_rate`, footgun #2) divided by its energy. Returns null when the
 * energy is non-positive (guard divide-by-zero). Shared by the Asset view's
 * orders table (both bid and offer components).
 */
export function perKwhFromComponent(c: DbOrderComponent): number | null {
  return c.energy > 0 ? c.energy_rate / c.energy : null;
}

/**
 * Per-kWh rate for a trade, derived from the bid's TOTAL price / bid energy.
 * Returns null when the bid energy is non-positive (guard divide-by-zero).
 */
export function perKwhRate(trade: TradeSchema): number | null {
  const e = trade.bid.bid_component.energy;
  return e > 0 ? trade.parameters.energy_rate / e : null;
}

export interface SlotAgg {
  time_slot: number;
  trade_count: number;
  /** Sum of selected_energy across the slot's trades (kWh). */
  volume: number;
  /** selected_energy-weighted mean per-kWh rate; null if no valid trades. */
  avgRatePerKwh: number | null;
  /** Trade with the maximum creation_time in the slot. */
  latestTrade: TradeSchema;
}

/**
 * Reduce trades into one aggregate per distinct `time_slot`, sorted ascending.
 *
 * - volume         = Σ selected_energy
 * - avgRatePerKwh  = Σ(rate_i · selected_energy_i) / Σ(selected_energy_i)
 *                    over trades whose perKwhRate is non-null; null if none.
 * - latestTrade    = trade with max creation_time
 * - trade_count    = number of trades in the slot
 */
export function perSlotAggregates(trades: TradeSchema[]): SlotAgg[] {
  const bySlot = new Map<number, TradeSchema[]>();
  for (const t of trades) {
    const bucket = bySlot.get(t.time_slot);
    if (bucket) bucket.push(t);
    else bySlot.set(t.time_slot, [t]);
  }

  const out: SlotAgg[] = [];
  for (const [time_slot, slotTrades] of bySlot) {
    let volume = 0;
    let weightedRateSum = 0;
    let ratedWeight = 0;
    let latestTrade = slotTrades[0];

    for (const t of slotTrades) {
      const energy = t.parameters.selected_energy;
      volume += energy;

      const rate = perKwhRate(t);
      if (rate !== null) {
        weightedRateSum += rate * energy;
        ratedWeight += energy;
      }

      if (t.creation_time > latestTrade.creation_time) latestTrade = t;
    }

    out.push({
      time_slot,
      trade_count: slotTrades.length,
      volume,
      avgRatePerKwh: ratedWeight > 0 ? weightedRateSum / ratedWeight : null,
      latestTrade,
    });
  }

  return out.sort((a, b) => a.time_slot - b.time_slot);
}
