// Trades in which a single asset participated (Milestone 5).
//
// Rows are pre-filtered in AssetView: a trade is kept when the asset's
// per-slot `area_hash` matched the offer side (role Seller) or the bid side
// (role Buyer) — see footgun #1. `counterparty` is the OTHER side's account
// key (footgun: trade.seller/buyer are accounts, never assets). The per-kWh
// column reuses `perKwhRate` (bid total ÷ bid energy, footgun #2).

import type { AssetTradeRow } from '../lib/asset';
import { formatSlot } from '../lib/time';
import { truncate } from '../lib/format';

interface Props {
  rows: AssetTradeRow[];
}

export default function AssetTradesTable({ rows }: Props) {
  if (rows.length === 0) {
    return <p className="muted">No trades for this asset in the window.</p>;
  }

  const sorted = [...rows].sort((a, b) => a.time_slot - b.time_slot);

  return (
    <div>
      <p className="caption muted">
        <code>role</code> is this asset's side; <code>counterparty</code> is the
        other side's account key (not an asset). per-kWh is{' '}
        <code>bid total ÷ bid energy</code>.
      </p>
      <table className="data-table">
        <thead>
          <tr>
            <th>Slot</th>
            <th>Role</th>
            <th>Counterparty</th>
            <th className="num">Sel. energy</th>
            <th className="num">per-kWh</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((r) => (
            <tr key={r.trade_uuid}>
              <td>{formatSlot(r.time_slot)}</td>
              <td>
                <span
                  className={
                    r.role === 'Seller' ? 'pill role-offer' : 'pill role-bid'
                  }
                >
                  {r.role}
                </span>
              </td>
              <td>
                {r.counterpartyName ? (
                  <div className="party-cell" title={r.counterparty}>
                    <span className="party-name">{r.counterpartyName}</span>
                    <span className="party-account mono muted">
                      {truncate(r.counterparty)}
                    </span>
                  </div>
                ) : (
                  <span className="mono" title={r.counterparty}>
                    {truncate(r.counterparty)}
                  </span>
                )}
              </td>
              <td className="num">{r.selected_energy}</td>
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
