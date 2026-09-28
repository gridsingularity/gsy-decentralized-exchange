// Per-slot aggregate table for the Market view. One row per time_slot.

import type { SlotAgg } from '../lib/aggregate';
import { formatSlot } from '../lib/time';
import { truncate } from '../lib/format';

interface Props {
  aggregates: SlotAgg[];
  /** time_slot of the grid-selected market, if any (row is highlighted). */
  selectedSlot: number | undefined;
}

export default function SlotAggregatesTable({
  aggregates,
  selectedSlot,
}: Props) {
  if (aggregates.length === 0) {
    return <p className="muted">No trades in this window.</p>;
  }
  return (
    <table className="data-table">
      <thead>
        <tr>
          <th>Slot</th>
          <th className="num">Trades</th>
          <th className="num">Volume (kWh)</th>
          <th className="num">Avg rate (price/kWh)</th>
          <th>Latest trade</th>
        </tr>
      </thead>
      <tbody>
        {aggregates.map((a) => (
          <tr
            key={a.time_slot}
            className={a.time_slot === selectedSlot ? 'selected' : undefined}
          >
            <td>{formatSlot(a.time_slot)}</td>
            <td className="num">{a.trade_count}</td>
            <td className="num">{a.volume.toFixed(3)}</td>
            <td className="num">
              {a.avgRatePerKwh === null ? '—' : a.avgRatePerKwh.toFixed(4)}
            </td>
            <td className="mono">{truncate(a.latestTrade.trade_uuid)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
