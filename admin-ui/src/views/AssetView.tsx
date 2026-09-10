// Asset view (Milestone 5).
//
// SCOPE: one asset identified by its stable `name`, tracked across the selected
// community's markets/slots in the window. Because `area_uuid`/`area_hash` are
// randomized PER MARKET, we resolve the asset per-slot (name → per-slot
// `{ time_slot, market_id, area_hash }`) and key every fetch/filter on that map.
//
// Data flow:
//   - Orders: one getOrders({ market_id }) per slot's market (Promise.all over
//     the ≤~6 markets), filtered to that market's area_hash (footgun #1).
//   - Trades: one getTrades({ start, end }) for the window, filtered to the
//     asset's set of per-slot area_hashes; the traded-energy series is summed
//     client-side from those trades.
//   - Measurements/forecasts: self-fetched by MeasurementForecastPanel.

import { useEffect, useMemo, useState } from 'react';
import { getOrders, getTradesResolved } from '../api/client';
import type { MarketTopologySchema } from '../api/schema';
import {
  assetOrderRows,
  assetTradeRows,
  resolveAssetSlots,
  tradedEnergySeries,
  type AssetOrderRow,
  type AssetTradeRow,
} from '../lib/asset';
import { formatSlot, type Window } from '../lib/time';
import { truncate } from '../lib/format';
import AssetOrdersTable from '../components/AssetOrdersTable';
import AssetTradesTable from '../components/AssetTradesTable';
import MeasurementForecastPanel from '../components/MeasurementForecastPanel';
import SlotBarChart, { type BarPoint } from '../components/SlotBarChart';
import { Empty, ErrorNote, Loading } from '../components/AsyncState';

interface Props {
  markets: MarketTopologySchema[]; // the community's markets (one per slot)
  assetName: string;
  window: Window;
}

interface OrdersState {
  loading: boolean;
  error: string | null;
  rows: AssetOrderRow[];
}

interface TradesState {
  loading: boolean;
  error: string | null;
  rows: AssetTradeRow[];
}

/** Short HH:mm label for a slot, used on chart x-axes. */
function slotLabel(sec: number): string {
  const d = new Date(sec * 1000);
  return `${String(d.getHours()).padStart(2, '0')}:${String(
    d.getMinutes(),
  ).padStart(2, '0')}`;
}

export default function AssetView({ markets, assetName, window }: Props) {
  const slots = useMemo(
    () => resolveAssetSlots(markets, assetName),
    [markets, assetName],
  );
  const communityName = markets[0]?.community_name ?? '';
  const areaType = slots[0]?.area_type;

  const [showSlots, setShowSlots] = useState(false);

  const [orders, setOrders] = useState<OrdersState>({
    loading: true,
    error: null,
    rows: [],
  });
  const [trades, setTrades] = useState<TradesState>({
    loading: true,
    error: null,
    rows: [],
  });

  // Orders: one call per market, filtered to that market's area_hash.
  useEffect(() => {
    let cancelled = false;
    setOrders({ loading: true, error: null, rows: [] });
    (async () => {
      try {
        const results = await Promise.all(
          slots.map((s) =>
            getOrders({ market_id: s.market_id }).then((o) =>
              assetOrderRows(o, s),
            ),
          ),
        );
        if (cancelled) return;
        setOrders({ loading: false, error: null, rows: results.flat() });
      } catch (err) {
        if (cancelled) return;
        setOrders({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          rows: [],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [slots]);

  // Trades: one call for the window, filtered to the asset's area_hashes.
  useEffect(() => {
    let cancelled = false;
    setTrades({ loading: true, error: null, rows: [] });
    (async () => {
      try {
        const all = await getTradesResolved({
          start_time: window.start,
          end_time: window.end,
        });
        if (cancelled) return;
        setTrades({
          loading: false,
          error: null,
          rows: assetTradeRows(all, slots),
        });
      } catch (err) {
        if (cancelled) return;
        setTrades({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          rows: [],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [slots, window.start, window.end]);

  const series = useMemo(
    () => tradedEnergySeries(trades.rows),
    [trades.rows],
  );
  const seriesPoints: BarPoint[] = series.map((p) => ({
    label: slotLabel(p.time_slot),
    value: p.value,
  }));

  if (slots.length === 0) {
    return (
      <section className="asset-view">
        <Empty>
          Asset “{assetName}” was not found in any market of “{communityName}” in
          the selected window.
        </Empty>
      </section>
    );
  }

  return (
    <section className="asset-view">
      <header className="market-header">
        <h2>
          {assetName}{' '}
          {areaType && <span className="topo-badge">{areaType}</span>}
        </h2>
        <dl className="market-stats">
          <div>
            <dt>Community</dt>
            <dd>{communityName}</dd>
          </div>
          <div>
            <dt>Slots appeared in</dt>
            <dd>{slots.length}</dd>
          </div>
          <div>
            <dt>Orders (window)</dt>
            <dd>{orders.loading ? '…' : orders.rows.length}</dd>
          </div>
          <div>
            <dt>Trades (window)</dt>
            <dd>{trades.loading ? '…' : trades.rows.length}</dd>
          </div>
        </dl>
        <button
          type="button"
          className="link-btn"
          onClick={() => setShowSlots((v) => !v)}
        >
          {showSlots ? '▾' : '▸'} per-slot identities ({slots.length}) — hash is
          randomized per market
        </button>
        {showSlots && (
          <table className="data-table asset-slots">
            <thead>
              <tr>
                <th>Slot</th>
                <th>market_id</th>
                <th>area_hash</th>
              </tr>
            </thead>
            <tbody>
              {slots.map((s) => (
                <tr key={s.market_id}>
                  <td>{formatSlot(s.time_slot)}</td>
                  <td className="mono">{truncate(s.market_id)}</td>
                  <td className="mono">{truncate(s.area_hash)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </header>

      <h3>Orders</h3>
      {orders.loading && <Loading label="Loading orders…" />}
      {orders.error && <ErrorNote error={orders.error} />}
      {!orders.loading && !orders.error && (
        <AssetOrdersTable rows={orders.rows} />
      )}

      <h3>Trades</h3>
      {trades.loading && <Loading label="Loading trades…" />}
      {trades.error && <ErrorNote error={trades.error} />}
      {!trades.loading && !trades.error && (
        <AssetTradesTable rows={trades.rows} />
      )}

      <h3>Traded energy per slot</h3>
      {trades.loading && <Loading />}
      {trades.error && <ErrorNote error={trades.error} />}
      {!trades.loading && !trades.error && (
        <>
          <SlotBarChart
            points={seriesPoints}
            title="Traded energy per slot"
            unit="kWh"
          />
          <p className="caption muted">
            Summed client-side from this asset's filtered trades (Σ
            <code>selected_energy</code> per slot). Equals the server's{' '}
            <code>/traded-energy?id=&lt;area_hash&gt;</code> per slot; derived
            here to avoid one call per per-slot area_hash.
          </p>
        </>
      )}

      <h3>Measurements & forecasts</h3>
      <MeasurementForecastPanel assetName={assetName} window={window} />
    </section>
  );
}
