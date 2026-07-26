// Raw (community-scoped) trades table with expandable rows.
//
// The `energy_rate` column is the BID'S TOTAL price (energy × rate), NOT a
// per-kWh rate — see the caption. The per-kWh column derives it as
// `energy_rate ÷ bid energy`; a large gap between the two is a debugging signal
// that the trade's stored price was not rescaled to the traded energy.

import { Fragment, useState } from 'react';
import type { DbBid, DbOffer, TradeCanonicalSchema } from '../api/schema';
import { perKwhRate } from '../lib/aggregate';
import { formatSlot } from '../lib/time';
import { truncate } from '../lib/format';

interface Props {
  trades: TradeCanonicalSchema[];
}

/**
 * Party cell: shows the resolved asset name as the primary label with the raw
 * SS58 account as a muted subtitle (and full-account tooltip). Falls back to the
 * truncated account when no name resolved.
 */
function PartyCell({ name, account }: { name: string | null; account: string }) {
  if (name) {
    return (
      <div className="party-cell" title={account}>
        <span className="party-name">{name}</span>
        <span className="party-account mono muted">{truncate(account)}</span>
      </div>
    );
  }
  return (
    <span className="mono" title={account}>
      {truncate(account)}
    </span>
  );
}

export default function RawTradesTable({ trades }: Props) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  if (trades.length === 0) {
    return <p className="muted">No trades to show.</p>;
  }

  const sorted = [...trades].sort((a, b) => a.time_slot - b.time_slot);

  function toggle(id: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <div>
      <p className="caption muted">
        <code>energy_rate</code> is the bid's total price (energy×rate), not
        per-kWh; the per-kWh column is <code>energy_rate ÷ bid energy</code>.
      </p>
      <table className="data-table raw-trades">
        <thead>
          <tr>
            <th />
            <th>Slot</th>
            <th>Trade</th>
            <th>Seller</th>
            <th>Buyer</th>
            <th className="num">Sel. energy</th>
            <th className="num">energy_rate (bid total)</th>
            <th className="num">per-kWh</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((t) => {
            const isOpen = expanded.has(t.trade_uuid);
            const perKwh = perKwhRate(t);
            return (
              <Fragment key={t.trade_uuid}>
                <tr
                  className="raw-trade-row"
                  onClick={() => toggle(t.trade_uuid)}
                >
                  <td className="expander">{isOpen ? '▾' : '▸'}</td>
                  <td>{formatSlot(t.time_slot)}</td>
                  <td className="mono">{truncate(t.trade_uuid)}</td>
                  <td>
                    <PartyCell name={t.seller_name} account={t.seller} />
                  </td>
                  <td>
                    <PartyCell name={t.buyer_name} account={t.buyer} />
                  </td>
                  <td className="num">{t.parameters.selected_energy}</td>
                  <td className="num">{t.parameters.energy_rate}</td>
                  <td className="num">
                    {perKwh === null ? '—' : perKwh.toFixed(4)}
                  </td>
                  <td>{t.status}</td>
                </tr>
                {isOpen && (
                  <tr className="raw-trade-detail">
                    <td />
                    <td colSpan={8}>
                      <div className="trade-detail-grid">
                        <ComponentCard title="Bid" bid={t.bid} />
                        <ComponentCard title="Offer" offer={t.offer} />
                      </div>
                      <p className="muted residual-note">
                        residual_bid:{' '}
                        {t.residual_bid ? 'present' : 'none'} · residual_offer:{' '}
                        {t.residual_offer ? 'present' : 'none'}
                      </p>
                    </td>
                  </tr>
                )}
              </Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function ComponentCard({
  title,
  bid,
  offer,
}: {
  title: string;
  bid?: DbBid;
  offer?: DbOffer;
}) {
  const c = bid ? bid.bid_component : offer!.offer_component;
  const party = bid ? bid.buyer : offer!.seller;
  const nonce = bid ? bid.nonce : offer!.nonce;
  return (
    <div className="component-card">
      <h5>{title}</h5>
      <dl className="kv">
        <dt>{bid ? 'buyer' : 'seller'}</dt>
        <dd className="mono">{truncate(party)}</dd>
        <dt>area_uuid</dt>
        <dd className="mono">{truncate(c.area_uuid)}</dd>
        <dt>energy</dt>
        <dd>{c.energy}</dd>
        <dt>energy_rate</dt>
        <dd>{c.energy_rate}</dd>
        <dt>nonce</dt>
        <dd>{nonce}</dd>
      </dl>
    </div>
  );
}
