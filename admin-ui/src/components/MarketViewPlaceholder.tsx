// Placeholder for the Market view (Milestone 4). Kept intentionally minimal
// and self-contained so M4 can replace this single component with the real
// aggregated trades/charts view without touching the grid view around it.

import type { MarketTopologySchema } from '../api/schema';
import { formatSlot } from '../lib/time';

interface Props {
  market: MarketTopologySchema;
}

export default function MarketViewPlaceholder({ market }: Props) {
  return (
    <section className="market-placeholder">
      <h2>Market view (M4)</h2>
      <dl className="kv">
        <dt>market_id</dt>
        <dd className="mono">{market.market_id}</dd>
        <dt>time_slot</dt>
        <dd>
          {market.time_slot}{' '}
          <span className="muted">({formatSlot(market.time_slot)})</span>
        </dd>
        <dt>areas</dt>
        <dd>{market.community_areas.length}</dd>
      </dl>
      <ul className="area-name-list">
        {market.community_areas.map((a) => (
          <li key={a.name}>
            <span className="topo-badge">{a.area_type}</span> {a.name}
          </li>
        ))}
      </ul>
      <p className="muted">
        This panel is a stub — Milestone 4 replaces it with the aggregated
        trades, per-slot rate/volume charts, and raw trades table.
      </p>
    </section>
  );
}
