// Orders placed by a single asset, across the community's slots (Milestone 5).
//
// Rows are pre-filtered in AssetView: only orders whose bid/offer component
// `area_uuid` equals that market's `area_hash` (footgun #1) reach here. The
// `energy_rate` column is the component's TOTAL price (footgun #2); the per-kWh
// column derives it as `energy_rate ÷ energy`.

import type { AssetOrderRow } from '../lib/asset';
import { formatSlot } from '../lib/time';

interface Props {
  rows: AssetOrderRow[];
}

export default function AssetOrdersTable({ rows }: Props) {
  if (rows.length === 0) {
    return <p className="muted">No orders for this asset in the window.</p>;
  }

  const sorted = [...rows].sort((a, b) => a.time_slot - b.time_slot);

  return (
    <div>
      <p className="caption muted">
        <code>energy_rate</code> is the order's total price (energy×rate), not
        per-kWh; the per-kWh column is <code>energy_rate ÷ energy</code>.
      </p>
      <table className="data-table">
        <thead>
          <tr>
            <th>Slot</th>
            <th>Side</th>
            <th className="num">Energy</th>
            <th className="num">energy_rate (bid/offer total)</th>
            <th className="num">per-kWh</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((r) => (
            <tr key={r.id}>
              <td>{formatSlot(r.time_slot)}</td>
              <td>
                <span
                  className={
                    r.side === 'Bid' ? 'pill role-bid' : 'pill role-offer'
                  }
                >
                  {r.side}
                </span>
              </td>
              <td className="num">{r.energy}</td>
              <td className="num">{r.energyRateTotal}</td>
              <td className="num">
                {r.perKwh === null ? '—' : r.perKwh.toFixed(4)}
              </td>
              <td>{r.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
